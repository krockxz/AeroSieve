use aho_corasick::AhoCorasick;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSet {
    pub version: u32,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub pattern: String,
    pub replacement: String,
    #[serde(default)]
    pub action: RuleAction,
    #[serde(default)]
    pub category: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RuleAction {
    #[default]
    Replace,
    Format,
    Remove,
    Prefix,
    Suffix,
}

#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub action: RuleAction,
    pub regex: Regex,
    pub replacement: String,
}

#[derive(Debug)]
pub struct RuleEngine {
    rules: Vec<CompiledRule>,
    keywords: Option<AhoCorasick>,
    keyword_map: Vec<Vec<usize>>,
    always_check: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct NormalizedText {
    pub original: String,
    pub normalized: String,
    pub rules_applied: Vec<String>,
}

impl RuleEngine {
    pub fn from_yaml_file(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        Self::from_yaml(&content)
    }

    pub fn from_yaml(yaml: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let rule_set: RuleSet = serde_yaml::from_str(yaml)?;
        Self::from_rule_set(&rule_set)
    }

    pub fn from_rule_set(rule_set: &RuleSet) -> Result<Self, Box<dyn std::error::Error>> {
        let mut rules = Vec::with_capacity(rule_set.rules.len());
        let mut keyword_map: HashMap<String, Vec<usize>> = HashMap::new();
        let mut always_check = Vec::new();

        for (i, rule) in rule_set.rules.iter().enumerate() {
            let regex = Regex::new(&rule.pattern)?;
            rules.push(CompiledRule {
                action: rule.action,
                regex,
                replacement: rule.replacement.clone(),
            });

            if let Some(keyword) = extract_keyword(&rule.pattern) {
                keyword_map.entry(keyword).or_default().push(i);
            } else {
                always_check.push(i);
            }
        }

        let keywords: Vec<String> = keyword_map.keys().cloned().collect();
        let keyword_patterns: Vec<&str> = keywords.iter().map(|s| s.as_str()).collect();
        let ac = AhoCorasick::new(keyword_patterns)?;

        let keyword_map: Vec<Vec<usize>> = keywords
            .iter()
            .map(|k| keyword_map.get(k).cloned().unwrap_or_default())
            .collect();

        Ok(Self {
            rules,
            keywords: Some(ac),
            keyword_map,
            always_check,
        })
    }

    pub fn empty() -> Self {
        Self {
            rules: Vec::new(),
            keywords: None,
            keyword_map: Vec::new(),
            always_check: Vec::new(),
        }
    }

    pub fn normalize(&self, text: &str) -> NormalizedText {
        let mut result = text.to_string();
        let mut rules_applied = Vec::new();

        let triggered = self.match_rules(text);
        for idx in triggered {
            let rule = &self.rules[idx];
            let prev = result.clone();
            result = match rule.action {
                RuleAction::Remove => rule.regex.replace_all(&result, "").to_string(),
                RuleAction::Prefix => {
                    let repl = format!("{}{}", rule.replacement, "$0");
                    rule.regex.replace_all(&result, &repl).to_string()
                }
                RuleAction::Suffix => {
                    let repl = format!("{}{}", "$0", rule.replacement);
                    rule.regex.replace_all(&result, &repl).to_string()
                }
                _ => rule.regex.replace_all(&result, &rule.replacement).to_string(),
            };
            if result != prev {
                rules_applied.push(rule.regex.as_str().to_string());
            }
        }

        NormalizedText {
            original: text.to_string(),
            normalized: result,
            rules_applied,
        }
    }

    fn match_rules(&self, text: &str) -> Vec<usize> {
        let mut candidates = Vec::new();
        if let Some(ref ac) = self.keywords {
            for mat in ac.find_iter(text) {
                let pattern_id = mat.pattern().as_usize();
                if pattern_id < self.keyword_map.len() {
                    candidates.extend_from_slice(&self.keyword_map[pattern_id]);
                }
            }
        }
        candidates.extend_from_slice(&self.always_check);
        if candidates.is_empty() {
            candidates = (0..self.rules.len()).collect();
        }
        candidates.sort();
        candidates.dedup();

        candidates
            .into_iter()
            .filter(|&idx| self.rules[idx].regex.is_match(text))
            .collect()
    }
}

fn extract_keyword(pattern: &str) -> Option<String> {
    let cleaned: String = pattern
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect();
    for word in cleaned.split_whitespace() {
        if word.len() >= 2 {
            let lower = word.to_lowercase();
            let chars: Vec<char> = lower.chars().collect();
            if chars.windows(2).any(|w| w[0].is_alphabetic() && w[1].is_alphabetic()) {
                return Some(lower);
            }
        }
    }
    None
}



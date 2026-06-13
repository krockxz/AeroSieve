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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RuleAction {
    Replace,
    Format,
    Remove,
    Prefix,
    Suffix,
}

impl Default for RuleAction {
    fn default() -> Self {
        Self::Replace
    }
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
    keywords: AhoCorasick,
    keyword_map: Vec<Vec<usize>>,
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
        let mut patterns = Vec::with_capacity(rule_set.rules.len());
        let mut keyword_map: HashMap<String, Vec<usize>> = HashMap::new();

        for (i, rule) in rule_set.rules.iter().enumerate() {
            let regex = Regex::new(&rule.pattern)?;
            rules.push(CompiledRule {
                action: rule.action,
                regex,
                replacement: rule.replacement.clone(),
            });

            if let Some(keyword) = extract_keyword(&rule.pattern) {
                keyword_map.entry(keyword).or_default().push(i);
            }

            patterns.push(rule.pattern.clone());
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
            keywords: ac,
            keyword_map,
        })
    }

    pub fn empty() -> Self {
        Self {
            rules: Vec::new(),
            keywords: AhoCorasick::new(&[""]).expect("empty AC"),
            keyword_map: Vec::new(),
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
                RuleAction::Replace => rule.regex.replace_all(&result, &rule.replacement).to_string(),
                RuleAction::Remove => rule.regex.replace_all(&result, "").to_string(),
                RuleAction::Format => rule.regex.replace_all(&result, &rule.replacement).to_string(),
                RuleAction::Prefix => format!("{}{}", rule.replacement, &result),
                RuleAction::Suffix => format!("{}{}", &result, rule.replacement),
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
        if !self.keyword_map.is_empty() {
            for mat in self.keywords.find_iter(text) {
                let pattern_id = mat.pattern().as_usize();
                if pattern_id < self.keyword_map.len() {
                    candidates.extend_from_slice(&self.keyword_map[pattern_id]);
                }
            }
        }
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
    let keyword = pattern
        .chars()
        .filter(|c| c.is_alphabetic())
        .take(20)
        .collect::<String>();
    if keyword.len() >= 2 {
        Some(keyword.to_lowercase())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_replacement() {
        let yaml = r#"
version: 1
rules:
  - pattern: '\u20B9\s*(\d+)'
    replacement: '$1 rupaye'
    action: Replace
    category: currency
"#;
        let engine = RuleEngine::from_yaml(yaml).unwrap();
        let result = engine.normalize("yeh \u{20B9}500 hai");
        assert_eq!(result.normalized, "yeh 500 rupaye hai");
        assert_eq!(result.rules_applied.len(), 1);
    }

    #[test]
    fn test_multiple_rules() {
        let yaml = r#"
version: 1
rules:
  - pattern: '\u20B9\s*(\d+)'
    replacement: '$1 rupaye'
    action: Replace
    category: currency
  - pattern: '(\d+)\s*km'
    replacement: '$1 kilometer'
    action: Replace
    category: distance
"#;
        let engine = RuleEngine::from_yaml(yaml).unwrap();
        let result = engine.normalize("\u{20B9}100 ke liye 5km");
        assert!(result.normalized.contains("100 rupaye"));
        assert!(result.normalized.contains("5 kilometer"));
    }

    #[test]
    fn test_remove_action() {
        let yaml = r#"
version: 1
rules:
  - pattern: '<[^>]+>'
    replacement: ''
    action: Remove
    category: markup
"#;
        let engine = RuleEngine::from_yaml(yaml).unwrap();
        let result = engine.normalize("hello <b>world</b>");
        assert_eq!(result.normalized, "hello world");
    }

    #[test]
    fn test_no_matching_rules() {
        let yaml = r#"
version: 1
rules:
  - pattern: '\u20B9\d+'
    replacement: 'rupees'
    action: Replace
    category: currency
"#;
        let engine = RuleEngine::from_yaml(yaml).unwrap();
        let result = engine.normalize("no currency here");
        assert_eq!(result.normalized, "no currency here");
        assert!(result.rules_applied.is_empty());
    }

    #[test]
    fn test_empty_engine() {
        let engine = RuleEngine::empty();
        let result = engine.normalize("hello world");
        assert_eq!(result.normalized, "hello world");
    }

    #[test]
    fn test_hindi_numerals() {
        let yaml = r#"
version: 1
rules:
  - pattern: '\b(\d+)\s*(?:lakh|laakh)\b'
    replacement: '$1 lakh'
    action: Format
    category: weights
  - pattern: '\b(\d+)\s*(?:crore|karod)\b'
    replacement: '$1 crore'
    action: Format
    category: weights
"#;
        let engine = RuleEngine::from_yaml(yaml).unwrap();
        let result = engine.normalize("5 laakh rupaye aur 2 karod");
        assert!(result.normalized.contains("5 lakh"));
        assert!(result.normalized.contains("2 crore"));
    }
}

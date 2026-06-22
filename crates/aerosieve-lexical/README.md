# aerosieve-lexical

Compiled rule engine for normalizing code-mixed Hinglish text.

- **Aho-Corasick** keyword pre-filter avoids running regexes on unrelated input
- Rules are defined in YAML and compiled once at startup
- Supports `Replace`, `Remove`, `Format`, `Prefix`, and `Suffix` actions
- Hot-reloadable (reload YAML without restarting pipeline)

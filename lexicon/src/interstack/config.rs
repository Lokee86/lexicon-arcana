use std::sync::LazyLock;

use regex::Regex;
use serde_json::json;

use crate::EdgeRecord;

use super::model::{Attributes, Node, SourceFile};
use super::paths::synthetic_path;
use super::resolver::{Resolver, attributes, line_span};

static CONFIG_READ_PATTERNS: LazyLock<Vec<(Regex, bool)>> = LazyLock::new(|| {
    [
        (
            r#"\bos\.(?:Getenv|LookupEnv)\(\s*([A-Za-z_][A-Za-z0-9_]*|["'][A-Za-z_][A-Za-z0-9_]*["'])"#,
            false,
        ),
        (
            r#"\bENV\s*\[\s*([A-Za-z_][A-Za-z0-9_]*|["'][A-Za-z_][A-Za-z0-9_]*["'])\s*\]"#,
            false,
        ),
        (
            r#"\bENV\.fetch\(\s*([A-Za-z_][A-Za-z0-9_]*|["'][A-Za-z_][A-Za-z0-9_]*["'])"#,
            false,
        ),
        (r"\bprocess\.env\.([A-Za-z_][A-Za-z0-9_]*)", true),
        (
            r#"\bOS\.get_environment\(\s*([A-Za-z_][A-Za-z0-9_]*|["'][A-Za-z_][A-Za-z0-9_]*["'])"#,
            false,
        ),
        (
            r#"\bos\.(?:getenv|environ\.get)\(\s*([A-Za-z_][A-Za-z0-9_]*|["'][A-Za-z_][A-Za-z0-9_]*["'])"#,
            false,
        ),
    ]
    .into_iter()
    .map(|(pattern, direct)| (Regex::new(pattern).unwrap(), direct))
    .collect()
});

const BOUNDARY_CONFIG_KEYS: &[&str] = &[
    "LEXICON_STATE_DIR",
    "GRIMOIRE_LEXICON_COMMAND",
    "GRIMOIRE_ARCANA_COMMAND",
    "GRIMOIRE_HOME",
];

impl Resolver<'_> {
    pub(crate) fn detect_config_reads(&mut self, file: &SourceFile) {
        for (index, line) in file.lines.iter().enumerate() {
            let Some(owner) = self.index.owner_at(&file.path, (index + 1) as u64) else {
                continue;
            };
            for (pattern, direct) in CONFIG_READ_PATTERNS.iter() {
                for capture in pattern.captures_iter(line) {
                    let Some(key) = self.resolve_config_key(&capture[1], *direct) else {
                        continue;
                    };
                    let node = self.add_config_key(&key);
                    self.add_edge(EdgeRecord {
                        attributes: attributes([
                            ("confidence", json!(1.0)),
                            ("evidence", json!([line.trim()])),
                            ("transport", json!("configuration")),
                        ]),
                        owner: None,
                        relation: "reads-config".into(),
                        source: owner.id.clone(),
                        span: Some(line_span(&file.path, index + 1, line)),
                        target: node.id,
                    });
                }
            }
        }
    }

    pub(crate) fn detect_boundary_config(&mut self, file: &SourceFile) {
        for (index, line) in file.lines.iter().enumerate() {
            let Some(owner) = self.index.owner_at(&file.path, (index + 1) as u64) else {
                continue;
            };
            for key in BOUNDARY_CONFIG_KEYS {
                let double = format!("\"{key}\"");
                let single = format!("'{key}'");
                if !line.contains(&double) && !line.contains(&single) {
                    continue;
                }
                let node = self.add_config_key(key);
                self.add_edge(EdgeRecord {
                    attributes: attributes([
                        ("confidence", json!(1.0)),
                        ("evidence", json!([line.trim()])),
                        ("transport", json!("boundary-configuration")),
                    ]),
                    owner: None,
                    relation: "reads-config".into(),
                    source: owner.id.clone(),
                    span: Some(line_span(&file.path, index + 1, line)),
                    target: node.id,
                });
            }
        }
    }

    fn resolve_config_key(&self, token: &str, direct: bool) -> Option<String> {
        let token = token.trim();
        if token.len() >= 2
            && ((token.starts_with('"') && token.ends_with('"'))
                || (token.starts_with('\'') && token.ends_with('\'')))
        {
            return Some(token[1..token.len() - 1].to_owned());
        }
        self.unique_string(token).or_else(|| {
            direct
                .then(|| token.to_owned())
                .filter(|value| !value.is_empty())
        })
    }

    pub(crate) fn add_config_key(&mut self, name: &str) -> Node {
        let identity = format!("config\0{name}");
        let id = crate::node_id(super::LANGUAGE, "config-key", &identity);
        if let Some(existing) = self.nodes.get(&id) {
            return existing.clone();
        }
        let mut node_attributes = Attributes::new();
        node_attributes.insert("key".into(), json!(name));
        node_attributes.insert("transport".into(), json!("configuration"));
        let node = Node {
            id,
            kind: "config-key".into(),
            name: name.to_owned(),
            path: synthetic_path("config", &identity),
            qualified_name: format!("config:{name}"),
            span: None,
            attributes: node_attributes,
        };
        self.add_node(node.clone());
        self.result.summary.config_keys += 1;
        node
    }
}

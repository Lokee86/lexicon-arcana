use std::sync::LazyLock;

use regex::Regex;
use serde_json::json;

use crate::EdgeRecord;

use super::model::{Attributes, Node, SourceFile};
use super::paths::{last_identifier, synthetic_path};
use super::resolver::{Resolver, attributes, line_span};

static INDEXED_TYPE_ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"\[\s*(?:FIELD_TYPE|["']type["'])\s*\]\s*=\s*([A-Za-z_][A-Za-z0-9_\.:]*|["'][^"']+["'])"#,
    )
    .unwrap()
});
static STRUCT_TYPE_ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\bType\s*:\s*([A-Za-z_][A-Za-z0-9_\.:]*|["'][^"']+["'])"#).unwrap()
});
static MAP_TYPE_ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"["']type["']\s*:\s*([A-Za-z_][A-Za-z0-9_\.:]*|["'][^"']+["'])"#).unwrap()
});
static SWITCH_CASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*case\s+(.+?)\s*:").unwrap());
static GDSCRIPT_MATCH_BRANCH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(TYPE_[A-Z0-9_]+)\s*:").unwrap());
static REGISTRATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?i)\b(?:register_(?:handler|message|packet)|subscribe_(?:message|packet)|on_(?:message|packet)|handle_(?:message|packet))\s*\(\s*([A-Za-z_][A-Za-z0-9_\.:]*|["'][^"']+["'])"#,
    )
    .unwrap()
});

impl Resolver<'_> {
    pub(crate) fn detect_message_producers(&mut self, file: &SourceFile) {
        for (index, line) in file.lines.iter().enumerate() {
            let tokens = message_assignment_tokens(line);
            if tokens.is_empty() {
                continue;
            }
            let Some(owner) = self.index.owner_at(&file.path, (index + 1) as u64) else {
                continue;
            };
            for token in tokens {
                let Some(value) = self.resolve_message_value(&token) else {
                    continue;
                };
                if !looks_like_message_value(&value) {
                    continue;
                }
                let channel = self.add_message_channel(&value);
                self.add_edge(EdgeRecord {
                    attributes: attributes([
                        ("confidence", json!(1.0)),
                        ("evidence", json!([line.trim()])),
                        ("transport", json!("packet")),
                    ]),
                    owner: None,
                    relation: "publishes".into(),
                    source: owner.id.clone(),
                    span: Some(line_span(&file.path, index + 1, line)),
                    target: channel.id,
                });
                self.result.summary.message_links += 1;
            }
        }
    }

    pub(crate) fn detect_message_consumers(&mut self, file: &SourceFile) {
        let mut packet_dispatch_until = None::<usize>;
        for (index, line) in file.lines.iter().enumerate() {
            let trimmed = line.trim().to_ascii_lowercase();
            if (trimmed.contains("switch ") || trimmed.contains("match "))
                && (trimmed.contains(".type")
                    || trimmed.contains("[\"type\"]")
                    || trimmed.contains("packet"))
            {
                packet_dispatch_until = Some(index + 120);
            }
            let mut tokens = Vec::new();
            if packet_dispatch_until.is_some_and(|until| index <= until) {
                if let Some(capture) = SWITCH_CASE.captures(line) {
                    tokens.extend(capture[1].split(',').map(str::to_owned));
                }
                if let Some(capture) = GDSCRIPT_MATCH_BRANCH.captures(line) {
                    tokens.push(capture[1].to_owned());
                }
            }
            if let Some(capture) = REGISTRATION.captures(line) {
                tokens.push(capture[1].to_owned());
            }
            if tokens.is_empty() {
                continue;
            }
            let Some(owner) = self.index.owner_at(&file.path, (index + 1) as u64) else {
                continue;
            };
            for token in tokens {
                let Some(value) = self.resolve_message_value(&token) else {
                    continue;
                };
                if !looks_like_message_value(&value) {
                    continue;
                }
                let channel = self.add_message_channel(&value);
                self.add_edge(EdgeRecord {
                    attributes: attributes([
                        ("confidence", json!(1.0)),
                        ("evidence", json!([line.trim()])),
                        ("transport", json!("packet")),
                    ]),
                    owner: None,
                    relation: "consumes".into(),
                    source: channel.id,
                    span: Some(line_span(&file.path, index + 1, line)),
                    target: owner.id.clone(),
                });
                self.result.summary.message_links += 1;
            }
        }
    }

    fn resolve_message_value(&self, token: &str) -> Option<String> {
        let token = token
            .trim()
            .trim_matches(|character| character == '"' || character == '\'');
        if token.is_empty() {
            return None;
        }
        if token.contains('_') && token.to_ascii_lowercase() == token {
            return Some(token.to_owned());
        }
        let name = last_identifier(token);
        (!name.is_empty())
            .then(|| self.unique_string(&name))
            .flatten()
    }

    fn add_message_channel(&mut self, value: &str) -> Node {
        let identity = format!("packet\0{value}");
        let id = crate::node_id(super::LANGUAGE, "message-channel", &identity);
        if let Some(existing) = self.nodes.get(&id) {
            return existing.clone();
        }
        let mut node_attributes = Attributes::new();
        node_attributes.insert("message".into(), json!(value));
        node_attributes.insert("transport".into(), json!("packet"));
        let node = Node {
            id,
            kind: "message-channel".into(),
            name: value.to_owned(),
            path: synthetic_path("messages", &identity),
            qualified_name: format!("packet:{value}"),
            span: None,
            attributes: node_attributes,
        };
        self.add_node(node.clone());
        self.result.summary.message_channels += 1;
        node
    }
}

fn message_assignment_tokens(line: &str) -> Vec<String> {
    [
        &*INDEXED_TYPE_ASSIGNMENT,
        &*STRUCT_TYPE_ASSIGNMENT,
        &*MAP_TYPE_ASSIGNMENT,
    ]
    .into_iter()
    .filter_map(|pattern| pattern.captures(line).map(|capture| capture[1].to_owned()))
    .collect()
}

fn looks_like_message_value(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 160
        || value
            .chars()
            .any(|character| matches!(character, '/' | ' ' | '\\' | ':'))
        || value.chars().any(|character| character.is_uppercase())
    {
        return false;
    }
    let value = value.to_ascii_lowercase();
    [
        "_request",
        "-request",
        ".request",
        "_response",
        "-response",
        ".response",
        "_event",
        "-event",
        ".event",
        "_command",
        "-command",
        ".command",
        "_message",
        "-message",
        ".message",
        "_packet",
        "-packet",
        ".packet",
    ]
    .iter()
    .any(|suffix| value.ends_with(suffix))
}

use crate::SourceSpan;
use regex::Regex;
use std::sync::OnceLock;
use tree_sitter::Node;

pub fn node_text<'a>(node: Node<'_>, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or_default()
}

pub fn named_children(node: Node<'_>) -> Vec<Node<'_>> {
    (0..node.named_child_count())
        .filter_map(|index| node.named_child(index))
        .collect()
}

pub fn first_descendant<'a>(node: Node<'a>, kinds: &[&str]) -> Option<Node<'a>> {
    if kinds.contains(&node.kind()) {
        return Some(node);
    }
    for child in named_children(node) {
        if let Some(found) = first_descendant(child, kinds) {
            return Some(found);
        }
    }
    None
}

pub fn span(path: &str, node: Node<'_>) -> SourceSpan {
    let start = node.start_position();
    let end = node.end_position();
    SourceSpan {
        path: path.into(),
        start_line: start.row as u64 + 1,
        start_column: start.column as u64 + 1,
        end_line: end.row as u64 + 1,
        end_column: end.column as u64 + 1,
    }
}

pub fn normalize_space(value: &str) -> String {
    whitespace().replace_all(value.trim(), " ").into_owned()
}

pub fn normalize_qualified(value: &str) -> String {
    value.trim().trim_start_matches("::").replace(' ', "")
}

pub fn qualify(scope: &str, name: &str) -> String {
    let raw = name.trim();
    let absolute = raw.starts_with("::");
    let name = normalize_qualified(raw);
    if name.is_empty()
        || scope.is_empty()
        || absolute
        || name == scope
        || name.starts_with(&format!("{scope}::"))
    {
        name
    } else {
        format!("{scope}::{name}")
    }
}

pub fn last_qualified_part(value: &str) -> String {
    normalize_qualified(value)
        .rsplit("::")
        .next()
        .unwrap_or_default()
        .into()
}

pub fn is_expression_kind(kind: &str) -> bool {
    kind.ends_with("_expression") || matches!(kind, "initializer_list" | "argument_list")
}

pub fn anonymous_name(kind: &str, node: Node<'_>, source: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(normalize_space(node_text(node, source)).as_bytes());
    format!("(anonymous {kind} {})", hex_prefix(&digest[..6]))
}

fn hex_prefix(bytes: &[u8]) -> String {
    bytes.iter().map(|value| format!("{value:02x}")).collect()
}

fn whitespace() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| Regex::new(r"\s+").unwrap())
}

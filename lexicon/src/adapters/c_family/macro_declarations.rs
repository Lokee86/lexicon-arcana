use super::{
    declaration_helpers::add_declaration,
    model::{ExtractionContext, SourceFile},
    syntax::{node_text, normalize_space, qualify},
};
use regex::Regex;
use serde_json::{Map, json};
use std::sync::OnceLock;
use tree_sitter::Node;

const NON_CALL_TARGETS: &[&str] = &[
    "_Alignof",
    "_Generic",
    "_Static_assert",
    "alignof",
    "defined",
    "do",
    "else",
    "for",
    "if",
    "return",
    "sizeof",
    "static_assert",
    "switch",
    "typeof",
    "typeof_unqual",
    "while",
];

pub fn handle_macro(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    let name = node
        .child_by_field_name("name")
        .map(|value| node_text(value, source).to_owned())
        .unwrap_or_default();
    if name.is_empty() {
        return;
    }

    let mut attributes = Map::new();
    attributes.insert("macro".into(), json!(true));
    attributes.insert(
        "function_like".into(),
        json!(node.kind() == "preproc_function_def"),
    );
    if context.conditional {
        attributes.insert("conditional".into(), json!(true));
    }
    if let Some(replacement) = macro_replacement(node, &name, source)
        && !replacement.is_empty()
    {
        if let Some(target) = direct_macro_target(&replacement) {
            attributes.insert("target".into(), json!(target));
        }
        attributes.insert("replacement".into(), json!(replacement));
    }

    add_declaration(
        file,
        node,
        context,
        "symbol",
        name.clone(),
        qualify(&context.container_qualified, &name),
        String::new(),
        false,
        true,
        attributes,
    );
}

pub fn is_include_guard(node: Node<'_>, source: &[u8]) -> bool {
    if !matches!(node.kind(), "preproc_ifdef" | "preproc_ifndef") {
        return false;
    }
    let text = node_text(node, source);
    let Some(captures) = include_guard_name().captures(text) else {
        return false;
    };
    let Some(name) = captures.get(1) else {
        return false;
    };
    let sample = &text[..text.len().min(2048)];
    let pattern = format!(r"(?m)^\s*#\s*define\s+{}\b", regex::escape(name.as_str()));
    Regex::new(&pattern).is_ok_and(|value| value.is_match(sample))
}

fn macro_replacement(node: Node<'_>, name: &str, source: &[u8]) -> Option<String> {
    let text = node_text(node, source);
    let index = text.find(name)?;
    let mut remainder = &text[index + name.len()..];
    if node.kind() == "preproc_function_def" {
        remainder = remainder.trim_start_matches([' ', '\t']);
        if remainder.starts_with('(') {
            let mut depth = 0usize;
            for (index, character) in remainder.char_indices() {
                match character {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            remainder = &remainder[index + 1..];
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Some(normalize_space(
        &remainder.replace("\\\r\n", " ").replace("\\\n", " "),
    ))
}

fn direct_macro_target(replacement: &str) -> Option<String> {
    let mut value = replacement.trim();
    while let Some(rest) = value.strip_prefix('(') {
        value = rest.trim_start();
    }
    let captures = macro_target().captures(value)?;
    let target = captures.get(1)?.as_str();
    (!NON_CALL_TARGETS.contains(&target)).then(|| target.into())
}

fn include_guard_name() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| Regex::new(r"^\s*#\s*ifndef\s+([A-Za-z_][A-Za-z0-9_]*)\b").unwrap())
}

fn macro_target() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| Regex::new(r"^([A-Za-z_][A-Za-z0-9_]*)\s*(\(|$)").unwrap())
}

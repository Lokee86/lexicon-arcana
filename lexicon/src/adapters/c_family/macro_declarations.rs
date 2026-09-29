use super::{
    declaration_helpers::add_declaration,
    macro_syntax::{direct_target, semantic_details},
    model::{ExtractionContext, SourceFile},
    syntax::{node_text, qualify},
};
use regex::Regex;
use serde_json::{Map, json};
use std::sync::OnceLock;
use tree_sitter::Node;

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

    let function_like = node.kind() == "preproc_function_def";
    let definition = node_text(node, source);
    let (replacement, parameters, calls) = semantic_details(definition, &name, function_like);
    let target = direct_target(&replacement);

    let mut attributes = Map::new();
    attributes.insert("macro".into(), json!(true));
    attributes.insert("function_like".into(), json!(function_like));
    if context.conditional {
        attributes.insert("conditional".into(), json!(true));
    }
    if !replacement.is_empty() {
        attributes.insert("replacement".into(), json!(replacement));
    }
    if !target.is_empty() {
        attributes.insert("target".into(), json!(target));
    }

    let index = add_declaration(
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
    file.declarations[index].macro_parameters = parameters;
    file.declarations[index].macro_calls = calls;
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
    let sample = bounded_prefix(text, 2048);
    let pattern = format!(r"(?m)^\s*#\s*define\s+{}\b", regex::escape(name.as_str()));
    Regex::new(&pattern).is_ok_and(|value| value.is_match(sample))
}

fn bounded_prefix(value: &str, max_bytes: usize) -> &str {
    let mut end = value.len().min(max_bytes);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn include_guard_name() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| Regex::new(r"^\s*#\s*ifndef\s+([A-Za-z_][A-Za-z0-9_]*)\b").unwrap())
}

#[cfg(test)]
mod tests {
    use super::bounded_prefix;

    #[test]
    fn bounded_prefix_never_splits_utf8() {
        let value = format!("{}─tail", "a".repeat(2046));
        let sample = bounded_prefix(&value, 2048);

        assert_eq!(sample.len(), 2046);
        assert_eq!(sample, "a".repeat(2046));
    }
}

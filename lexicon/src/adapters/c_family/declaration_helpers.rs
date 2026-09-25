use super::{
    discovery::is_header_path,
    model::{Declaration, ExtractionContext, SourceFile},
    syntax::{first_descendant, last_qualified_part, node_text, normalize_space, span},
};
use crate::node_id;
use serde_json::{Map, Value, json};
use tree_sitter::Node;

pub fn top_level_declarators(node: Node<'_>) -> Vec<Node<'_>> {
    super::syntax::named_children(node)
        .into_iter()
        .filter(|child| {
            !matches!(
                child.kind(),
                "primitive_type"
                    | "type_identifier"
                    | "sized_type_specifier"
                    | "struct_specifier"
                    | "union_specifier"
                    | "enum_specifier"
                    | "class_specifier"
                    | "storage_class_specifier"
                    | "type_qualifier"
                    | "attribute_specifier"
                    | "attribute_declaration"
                    | "access_specifier"
            ) && is_declarator_node(child.kind())
        })
        .collect()
}

pub fn declarator_name(node: Node<'_>, source: &[u8]) -> String {
    match node.kind() {
        "identifier"
        | "field_identifier"
        | "type_identifier"
        | "namespace_identifier"
        | "operator_name"
        | "destructor_name" => return node_text(node, source).into(),
        "qualified_identifier" | "scoped_identifier" => {
            return super::syntax::normalize_qualified(node_text(node, source));
        }
        _ => {}
    }
    for field in ["declarator", "name"] {
        if let Some(child) = node.child_by_field_name(field) {
            let name = declarator_name(child, source);
            if !name.is_empty() {
                return name;
            }
        }
    }
    for child in super::syntax::named_children(node) {
        let name = declarator_name(child, source);
        if !name.is_empty() {
            return name;
        }
    }
    String::new()
}

pub fn declaration_type(node: Node<'_>, source: &[u8]) -> String {
    if let Some(value) = node.child_by_field_name("type") {
        return normalize_space(node_text(value, source));
    }
    super::syntax::named_children(node)
        .into_iter()
        .find(|child| {
            matches!(
                child.kind(),
                "primitive_type"
                    | "type_identifier"
                    | "sized_type_specifier"
                    | "struct_specifier"
                    | "union_specifier"
                    | "enum_specifier"
                    | "class_specifier"
            )
        })
        .map(|child| normalize_space(node_text(child, source)))
        .unwrap_or_default()
}

pub fn has_storage_class(node: Node<'_>, source: &[u8], value: &str) -> bool {
    super::syntax::named_children(node)
        .into_iter()
        .any(|child| {
            child.kind() == "storage_class_specifier"
                && normalize_space(node_text(child, source)) == value
        })
}

pub fn is_function_pointer_declarator(node: Node<'_>) -> bool {
    node.child_by_field_name("declarator").is_some_and(|child| {
        first_descendant(
            child,
            &["pointer_declarator", "abstract_pointer_declarator"],
        )
        .is_some()
    })
}

#[allow(clippy::too_many_arguments)]
pub fn add_declaration(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    kind: &str,
    name: String,
    qualified: String,
    signature: String,
    callable: bool,
    definition: bool,
    mut attributes: Map<String, Value>,
) -> usize {
    let mut canonical = format!("{}::{kind}::{qualified}", file.path);
    if !signature.is_empty() {
        canonical.push_str("::");
        canonical.push_str(&signature);
    }
    attributes.insert("language".into(), json!(file.language));
    let file_local = attributes
        .get("linkage")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "internal");
    let macro_function = attributes
        .get("function_like")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let macro_target = attributes
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let declaration = Declaration {
        id: node_id("c-family", kind, &canonical),
        kind: kind.into(),
        name,
        qualified_name: qualified,
        path: file.path.clone(),
        container_id: context.container_id.clone(),
        container_qualified: context.container_qualified.clone(),
        parent_type_id: context.type_id.clone(),
        signature,
        file_language: file.language.clone(),
        span: span(&file.path, node),
        attributes,
        callable,
        definition,
        file_local,
        macro_function,
        macro_target,
        macro_parameters: Vec::new(),
        macro_calls: Vec::new(),
        callable_shape: None,
    };
    file.declarations.push(declaration);
    file.declarations.len() - 1
}

pub fn internal_linkage(
    file: &SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) -> bool {
    context.callable_id.is_empty()
        && context.type_id.is_empty()
        && !is_header_path(&file.path)
        && has_storage_class(node, source, "static")
}

pub fn local_signature(node: Node<'_>, context: &ExtractionContext) -> String {
    if context.callable_id.is_empty() {
        String::new()
    } else {
        format!("local@{}", node.start_byte())
    }
}

pub fn variable_name(node: Node<'_>, source: &[u8]) -> String {
    last_qualified_part(&declarator_name(node, source))
}

fn is_declarator_node(kind: &str) -> bool {
    matches!(
        kind,
        "identifier" | "field_identifier" | "type_identifier" | "init_declarator"
    ) || kind.ends_with("_declarator")
}

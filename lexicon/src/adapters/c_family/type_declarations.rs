use super::{
    declaration_helpers::add_declaration,
    declarations::walk,
    model::{ExtractionContext, InheritanceObservation, SourceFile},
    syntax::{
        anonymous_name, first_descendant, named_children, node_text, normalize_qualified, qualify,
        span,
    },
};
use serde_json::{Map, json};
use tree_sitter::Node;

pub fn handle_namespace(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    let name = node
        .child_by_field_name("name")
        .map(|value| node_text(value, source).to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| anonymous_name("namespace", node, source));
    let qualified = qualify(&context.container_qualified, &name);
    let index = add_declaration(
        file,
        node,
        context,
        "namespace",
        name,
        qualified,
        String::new(),
        false,
        true,
        Map::new(),
    );
    let declaration = file.declarations[index].clone();
    let mut nested = context.clone();
    nested.container_id = declaration.id;
    nested.container_qualified = declaration.qualified_name;
    if let Some(body) = node.child_by_field_name("body") {
        for child in named_children(body) {
            walk(file, child, &nested, source);
        }
    }
}

pub fn handle_type(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    let tag = node.kind().trim_end_matches("_specifier");
    let name = node
        .child_by_field_name("name")
        .map(|value| node_text(value, source).to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| anonymous_name(tag, node, source));
    let qualified = qualify(&context.container_qualified, &name);
    let mut attributes = Map::new();
    attributes.insert("tag".into(), json!(tag));
    let index = add_declaration(
        file,
        node,
        context,
        "type",
        name,
        qualified,
        String::new(),
        false,
        true,
        attributes,
    );
    let declaration = file.declarations[index].clone();
    add_inheritance(file, node, context, source, &declaration);

    let mut nested = context.clone();
    nested.container_id = declaration.id.clone();
    nested.container_qualified = declaration.qualified_name.clone();
    nested.type_id = declaration.id;
    nested.type_name = declaration.name;
    if let Some(body) = node.child_by_field_name("body") {
        for child in named_children(body) {
            if child.kind() != "access_specifier" {
                walk(file, child, &nested, source);
            }
        }
    }
}

pub fn extract_type_child(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    if matches!(
        node.kind(),
        "class_specifier" | "struct_specifier" | "union_specifier" | "enum_specifier"
    ) {
        handle_type(file, node, context, source);
    }
}

fn add_inheritance(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
    declaration: &super::model::Declaration,
) {
    let Some(base_clause) = first_descendant(node, &["base_class_clause"]) else {
        return;
    };
    for child in named_children(base_clause) {
        if child.kind() == "access_specifier" {
            continue;
        }
        let candidate = normalize_qualified(node_text(child, source));
        if candidate.is_empty() {
            continue;
        }
        file.inheritance.push(InheritanceObservation {
            source_id: declaration.id.clone(),
            source_scope: context.container_qualified.clone(),
            path: file.path.clone(),
            expression: node_text(child, source).into(),
            candidate,
            span: span(&file.path, child),
        });
    }
}

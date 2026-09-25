use super::{
    declaration_helpers::{add_declaration, declarator_name},
    model::{ExtractionContext, SourceFile},
    syntax::{first_descendant, last_qualified_part, node_text, normalize_space, qualify},
};
use serde_json::{Map, json};
use tree_sitter::Node;

pub fn handle_typedef(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    if let Some(declarator) = node.child_by_field_name("declarator") {
        let name = declarator_name(declarator, source);
        if !name.is_empty() {
            let mut attributes = Map::new();
            attributes.insert("alias".into(), json!(true));
            let target = node
                .child_by_field_name("type")
                .map(|value| normalize_space(node_text(value, source)))
                .unwrap_or_default();
            attributes.insert("target".into(), json!(target));
            if first_descendant(declarator, &["function_declarator"]).is_some() {
                attributes.insert("function_pointer".into(), json!(true));
            }
            add_declaration(
                file,
                node,
                context,
                "type",
                last_qualified_part(&name),
                qualify(&context.container_qualified, &name),
                String::new(),
                false,
                true,
                attributes,
            );
        }
    }

    if let Some(type_node) = node.child_by_field_name("type") {
        super::declarations::extract_type_child(file, type_node, context, source);
    }
}

pub fn handle_alias(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    let name = node
        .child_by_field_name("name")
        .or_else(|| first_descendant(node, &["type_identifier"]))
        .map(|value| node_text(value, source).to_owned())
        .unwrap_or_default();
    if name.is_empty() {
        return;
    }
    let mut attributes = Map::new();
    attributes.insert("alias".into(), json!(true));
    add_declaration(
        file,
        node,
        context,
        "type",
        name.clone(),
        qualify(&context.container_qualified, &name),
        String::new(),
        false,
        true,
        attributes,
    );
}

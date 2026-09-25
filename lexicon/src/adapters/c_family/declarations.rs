use super::{
    callables::{function_declarator_is_pointer, handle_function, handle_function_declarator},
    declaration_helpers::{
        add_declaration, declaration_type, internal_linkage, local_signature,
        top_level_declarators, variable_name,
    },
    model::{ExtractionContext, SourceFile},
    syntax::{first_descendant, named_children, node_text, qualify},
};
use serde_json::{Map, json};
use tree_sitter::Node;

pub fn extract(file: &mut SourceFile, root: Node<'_>) {
    let context = ExtractionContext {
        container_id: crate::node_id("c-family", "module", &file.path),
        ..Default::default()
    };
    let source = file.content.clone();
    walk(file, root, &context, &source);
}

pub(super) fn walk(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    match node.kind() {
        "namespace_definition" => {
            super::type_declarations::handle_namespace(file, node, context, source);
            return;
        }
        "class_specifier" | "struct_specifier" | "union_specifier" | "enum_specifier" => {
            super::type_declarations::handle_type(file, node, context, source);
            return;
        }
        "function_definition" => {
            handle_function_definition(file, node, context, source);
            return;
        }
        "declaration" | "field_declaration" => {
            handle_declaration(file, node, context, source);
            return;
        }
        "type_definition" => {
            super::declaration_records::handle_typedef(file, node, context, source);
            return;
        }
        "alias_declaration" => {
            super::declaration_records::handle_alias(file, node, context, source);
            return;
        }
        "preproc_include" => {
            super::includes::extract(file, node, source);
            return;
        }
        "preproc_if" | "preproc_ifdef" | "preproc_ifndef" | "preproc_elif" | "preproc_else" => {
            let mut nested = context.clone();
            if !super::macro_declarations::is_include_guard(node, source) {
                nested.conditional = true;
            }
            for child in named_children(node) {
                walk(file, child, &nested, source);
            }
            return;
        }
        "preproc_def" | "preproc_function_def" => {
            super::macro_declarations::handle_macro(file, node, context, source);
            return;
        }
        "enumerator" => {
            handle_enumerator(file, node, context, source);
            return;
        }
        "template_declaration" => {
            let mut nested = context.clone();
            nested.template = true;
            for child in named_children(node) {
                walk(file, child, &nested, source);
            }
            return;
        }
        "call_expression"
        | "assignment_expression"
        | "update_expression"
        | "field_expression"
        | "subscript_expression" => {
            if !context.callable_id.is_empty() {
                super::dataflow::extract_expression(file, node, context, source);
                return;
            }
        }
        "identifier" => {
            if !context.callable_id.is_empty() {
                super::dataflow::extract_expression(file, node, context, source);
            }
            return;
        }
        _ => {}
    }
    for child in named_children(node) {
        walk(file, child, context, source);
    }
}

fn handle_function_definition(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    let Some((index, declarator)) = handle_function(file, node, context, source, true) else {
        return;
    };
    let declaration = file.declarations[index].clone();
    let mut nested = context.clone();
    nested.container_id = declaration.id.clone();
    nested.container_qualified = declaration.qualified_name.clone();
    nested.callable_id = declaration.id;
    nested.callable_scope = declaration.qualified_name;
    for child in named_children(node) {
        if !same_node(child, declarator)
            && !matches!(
                child.kind(),
                "primitive_type" | "type_identifier" | "storage_class_specifier"
            )
        {
            walk(file, child, &nested, source);
        }
    }
}

fn handle_declaration(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    for declarator in top_level_declarators(node) {
        if let Some(function) = first_descendant(declarator, &["function_declarator"]) {
            if function_declarator_is_pointer(function)
                || (!context.type_id.is_empty() && file.language == "c")
            {
                handle_variable(file, node, declarator, context, source);
            } else {
                let _ = handle_function_declarator(file, node, function, context, source, false);
            }
        } else {
            handle_variable(file, node, declarator, context, source);
        }
    }
}

fn handle_variable(
    file: &mut SourceFile,
    node: Node<'_>,
    declarator: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    let name = variable_name(declarator, source);
    if name.is_empty() {
        return;
    }
    let kind = if context.callable_id.is_empty() && !context.type_id.is_empty() {
        "field"
    } else if node_text(node, source).contains("const")
        || node_text(node, source).contains("constexpr")
    {
        "constant"
    } else {
        "variable"
    };
    let mut attributes = Map::new();
    attributes.insert("type".into(), json!(declaration_type(node, source)));
    if internal_linkage(file, node, context, source) {
        attributes.insert("linkage".into(), json!("internal"));
    }
    if first_descendant(declarator, &["function_declarator"]).is_some() {
        attributes.insert("function_pointer".into(), json!(true));
        if let Some(value) = declarator.child_by_field_name("value") {
            let target = super::call_references::callable_reference_candidate(value, source);
            if !target.is_empty() {
                attributes.insert("pointer_target".into(), json!(target));
            }
        }
    }
    let index = add_declaration(
        file,
        declarator,
        context,
        kind,
        name.clone(),
        qualify(&context.container_qualified, &name),
        local_signature(declarator, context),
        false,
        true,
        attributes,
    );
    super::pointer_bindings::collect_designated(file, declarator, context, source);
    let declaration = file.declarations[index].clone();
    super::dataflow::record_initializer(file, declarator, &declaration, context, source);
}

fn handle_enumerator(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    let name = node
        .child_by_field_name("name")
        .or_else(|| first_descendant(node, &["identifier"]))
        .map(|value| node_text(value, source).to_owned())
        .unwrap_or_default();
    if name.is_empty() {
        return;
    }
    let mut attributes = Map::new();
    attributes.insert("enum_member".into(), json!(true));
    add_declaration(
        file,
        node,
        context,
        "constant",
        name.clone(),
        qualify(&context.container_qualified, &name),
        String::new(),
        false,
        true,
        attributes,
    );
}

fn same_node(left: Node<'_>, right: Node<'_>) -> bool {
    left.start_byte() == right.start_byte()
        && left.end_byte() == right.end_byte()
        && left.kind() == right.kind()
}

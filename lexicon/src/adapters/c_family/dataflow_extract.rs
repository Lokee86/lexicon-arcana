use super::{
    model::{AccessObservation, Declaration, ExtractionContext, SourceFile},
    syntax::{last_qualified_part, named_children, node_text, span},
};
use tree_sitter::Node;

pub fn extract_expression(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    if context.callable_id.is_empty() {
        return;
    }
    match node.kind() {
        "call_expression" => extract_call(file, node, context, source),
        "assignment_expression" => extract_assignment(file, node, context, source),
        "update_expression" => {
            if let Some(target) = first_expression_child(node) {
                collect_target(file, target, context, source, "reads");
                collect_target(file, target, context, source, "writes");
            }
        }
        "field_expression" => {
            if let Some(field) = node.child_by_field_name("field") {
                add_access(file, field, context, source, "reads", true);
            }
            if let Some(receiver) = node.child_by_field_name("argument") {
                extract_expression(file, receiver, context, source);
            }
        }
        "subscript_expression" => {
            if let Some(argument) = node.child_by_field_name("argument") {
                extract_expression(file, argument, context, source);
            }
            if let Some(index) = node.child_by_field_name("index") {
                extract_expression(file, index, context, source);
            }
        }
        "identifier" => add_access(file, node, context, source, "reads", false),
        "field_identifier" => add_access(file, node, context, source, "reads", true),
        "type_identifier" | "primitive_type" | "number_literal" | "string_literal"
        | "char_literal" | "true" | "false" | "null" | "nullptr" => {}
        _ => {
            for child in named_children(node) {
                extract_expression(file, child, context, source);
            }
        }
    }
}

pub fn record_initializer(
    file: &mut SourceFile,
    declarator: Node<'_>,
    declaration: &Declaration,
    context: &ExtractionContext,
    source: &[u8],
) {
    if context.callable_id.is_empty() || declarator.kind() != "init_declarator" {
        return;
    }
    file.accesses.push(AccessObservation {
        source_id: context.callable_id.clone(),
        source_scope: context.callable_scope.clone(),
        parent_type_id: context.type_id.clone(),
        path: file.path.clone(),
        expression: declaration.name.clone(),
        candidate: declaration.name.clone(),
        relation: "writes".into(),
        member: false,
        span: declaration.span.clone(),
    });
    if let Some(value) = declarator.child_by_field_name("value") {
        extract_expression(file, value, context, source);
    }
}

fn extract_call(file: &mut SourceFile, node: Node<'_>, context: &ExtractionContext, source: &[u8]) {
    super::expressions::observe(file, node, context, source);
    if let Some(function) = node.child_by_field_name("function")
        && function.kind() == "field_expression"
        && let Some(receiver) = function.child_by_field_name("argument")
    {
        extract_expression(file, receiver, context, source);
    }
    if let Some(arguments) = node.child_by_field_name("arguments") {
        for child in named_children(arguments) {
            extract_expression(file, child, context, source);
        }
    }
}

fn extract_assignment(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    let left = node.child_by_field_name("left");
    let right = node.child_by_field_name("right");
    if let Some(left) = left {
        collect_target(file, left, context, source, "writes");
    }
    super::pointer_bindings::collect_assignment(file, node, context, source);
    if let (Some(left), Some(right)) = (left, right)
        && assignment_operator(left, right, source) != "="
    {
        collect_target(file, left, context, source, "reads");
    }
    if let Some(right) = right {
        extract_expression(file, right, context, source);
    }
}

fn add_access(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
    relation: &str,
    member: bool,
) {
    let candidate = last_qualified_part(node_text(node, source));
    if candidate.is_empty() || matches!(candidate.as_str(), "this" | "self") {
        return;
    }
    file.accesses.push(AccessObservation {
        source_id: context.callable_id.clone(),
        source_scope: context.callable_scope.clone(),
        parent_type_id: context.type_id.clone(),
        path: file.path.clone(),
        expression: node_text(node, source).into(),
        candidate,
        relation: relation.into(),
        member,
        span: span(&file.path, node),
    });
}

fn collect_target(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
    relation: &str,
) {
    match node.kind() {
        "identifier" => add_access(file, node, context, source, relation, false),
        "field_identifier" => add_access(file, node, context, source, relation, true),
        "field_expression" => {
            if let Some(field) = node.child_by_field_name("field") {
                add_access(file, field, context, source, relation, true);
            }
            if let Some(receiver) = node.child_by_field_name("argument") {
                extract_expression(file, receiver, context, source);
            }
        }
        "subscript_expression" => {
            if let Some(argument) = node.child_by_field_name("argument") {
                collect_target(file, argument, context, source, relation);
            }
            if let Some(index) = node.child_by_field_name("index") {
                extract_expression(file, index, context, source);
            }
        }
        _ => {
            if let Some(argument) = node.child_by_field_name("argument") {
                collect_target(file, argument, context, source, relation);
            } else {
                for child in named_children(node) {
                    collect_target(file, child, context, source, relation);
                }
            }
        }
    }
}

fn assignment_operator<'a>(left: Node<'_>, right: Node<'_>, source: &'a [u8]) -> &'a str {
    if left.end_byte() > right.start_byte() {
        return "=";
    }
    std::str::from_utf8(&source[left.end_byte()..right.start_byte()])
        .unwrap_or("=")
        .trim()
}

fn first_expression_child(node: Node<'_>) -> Option<Node<'_>> {
    named_children(node)
        .into_iter()
        .find(|child| child.kind() != "comment")
}

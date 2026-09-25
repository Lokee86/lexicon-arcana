use super::{
    call_references::callable_reference_candidate,
    model::{CallObservation, ExtractionContext, SourceFile},
    syntax::{last_qualified_part, named_children, node_text, normalize_qualified, span},
};
use tree_sitter::Node;

pub fn observe(file: &mut SourceFile, node: Node<'_>, context: &ExtractionContext, source: &[u8]) {
    if node.kind() == "assignment_expression" {
        super::pointer_bindings::collect_assignment(file, node, context, source);
        return;
    }
    if context.callable_id.is_empty() || node.kind() != "call_expression" {
        return;
    }
    let Some(function) = node.child_by_field_name("function") else {
        return;
    };
    let (candidate, member, receiver) = call_candidate(function, source);
    if candidate.is_empty() {
        return;
    }
    let receiver_type_id = if !context.type_id.is_empty()
        && (!member || receiver == "this" || receiver == "self")
        && !candidate.contains("::")
    {
        context.type_id.clone()
    } else {
        String::new()
    };
    let argument_nodes = node
        .child_by_field_name("arguments")
        .map(named_children)
        .unwrap_or_default()
        .into_iter()
        .filter(|child| child.kind() != "comment")
        .collect::<Vec<_>>();
    let arguments = argument_nodes
        .iter()
        .map(|child| callable_reference_candidate(*child, source))
        .collect();
    let argument_expressions = argument_nodes
        .iter()
        .map(|child| node_text(*child, source).to_owned())
        .collect();

    file.calls.push(CallObservation {
        source_id: context.callable_id.clone(),
        source_scope: context.callable_scope.clone(),
        path: file.path.clone(),
        expression: node_text(function, source).into(),
        candidate,
        arguments,
        argument_expressions,
        member,
        receiver,
        receiver_type_id,
        span: span(&file.path, node),
    });
}

pub fn observe_tree(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    if matches!(node.kind(), "call_expression" | "assignment_expression") {
        observe(file, node, context, source);
    }
    for child in named_children(node) {
        observe_tree(file, child, context, source);
    }
}

fn call_candidate(node: Node<'_>, source: &[u8]) -> (String, bool, String) {
    match node.kind() {
        "identifier"
        | "type_identifier"
        | "qualified_identifier"
        | "scoped_identifier"
        | "operator_name" => (
            normalize_qualified(node_text(node, source)),
            false,
            String::new(),
        ),
        "field_expression" => {
            let candidate = node
                .child_by_field_name("field")
                .map(|field| last_qualified_part(node_text(field, source)))
                .unwrap_or_default();
            let receiver = node
                .child_by_field_name("argument")
                .map(|value| direct_receiver_name(value, source))
                .unwrap_or_default();
            (candidate, true, receiver)
        }
        "template_function" => {
            let candidate = node
                .child_by_field_name("name")
                .map(|name| normalize_qualified(node_text(name, source)))
                .unwrap_or_default();
            (candidate, false, String::new())
        }
        _ => (String::new(), false, String::new()),
    }
}

fn direct_receiver_name(node: Node<'_>, source: &[u8]) -> String {
    match node.kind() {
        "identifier" | "field_identifier" | "type_identifier" => {
            let name = normalize_qualified(node_text(node, source));
            if !name.contains(['.', '-', '>', '(', ')']) {
                name
            } else {
                String::new()
            }
        }
        "this" => "this".into(),
        _ => String::new(),
    }
}

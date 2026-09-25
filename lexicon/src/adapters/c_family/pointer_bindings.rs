use super::{
    call_references::callable_reference_candidate,
    model::{ExtractionContext, PointerBindingObservation, SourceFile},
    syntax::{first_descendant, last_qualified_part, named_children, node_text, span},
};
use tree_sitter::Node;

pub fn collect_assignment(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    if context.callable_id.is_empty() {
        return;
    }
    let (Some(left), Some(right)) = (
        node.child_by_field_name("left"),
        node.child_by_field_name("right"),
    ) else {
        return;
    };
    let target = callable_reference_candidate(right, source);
    if target.is_empty() {
        return;
    }
    let (candidate, member) = match left.kind() {
        "identifier" => (node_text(left, source).to_owned(), false),
        "field_expression" => (
            left.child_by_field_name("field")
                .map(|field| node_text(field, source).to_owned())
                .unwrap_or_default(),
            true,
        ),
        _ => (String::new(), false),
    };
    let candidate = last_qualified_part(&candidate);
    if candidate.is_empty() {
        return;
    }
    file.pointer_bindings.push(PointerBindingObservation {
        source_id: context.callable_id.clone(),
        source_scope: context.callable_scope.clone(),
        path: file.path.clone(),
        candidate,
        target,
        member,
        span: span(&file.path, left),
    });
}

pub fn collect_designated(
    file: &mut SourceFile,
    node: Node<'_>,
    context: &ExtractionContext,
    source: &[u8],
) {
    if node.kind() == "initializer_pair" {
        let field = node
            .child_by_field_name("designator")
            .and_then(|value| first_descendant(value, &["field_identifier"]));
        let target = node
            .child_by_field_name("value")
            .map(|value| callable_reference_candidate(value, source))
            .unwrap_or_default();
        if let Some(field) = field
            && !target.is_empty()
        {
            file.pointer_bindings.push(PointerBindingObservation {
                source_id: context.callable_id.clone(),
                source_scope: context.callable_scope.clone(),
                path: file.path.clone(),
                candidate: node_text(field, source).to_owned(),
                target,
                member: true,
                span: span(&file.path, node),
            });
        }
    }
    for child in named_children(node) {
        collect_designated(file, child, context, source);
    }
}

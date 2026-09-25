use super::syntax::{named_children, node_text, normalize_qualified};
use tree_sitter::Node;

pub fn callable_reference_candidate(node: Node<'_>, source: &[u8]) -> String {
    match node.kind() {
        "identifier"
        | "type_identifier"
        | "qualified_identifier"
        | "scoped_identifier"
        | "operator_name" => normalize_qualified(node_text(node, source)),
        "pointer_expression"
        | "unary_expression"
        | "parenthesized_expression"
        | "cast_expression" => {
            for child in named_children(node) {
                let candidate = callable_reference_candidate(child, source);
                if !candidate.is_empty() {
                    return candidate;
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

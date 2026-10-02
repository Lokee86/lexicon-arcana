use serde_json::{Value, json};

use crate::repository::{CatalogueEntry, RelationKind, SourceSpan, UnresolvedReferenceFact};
use crate::synthetic::NodeId;

use super::PROTOCOL_ID;

pub(crate) fn success(id: Value, result: Value) -> Value {
    json!({
        "protocol": PROTOCOL_ID,
        "id": id,
        "ok": true,
        "result": result,
    })
}

pub(crate) fn failure(id: Value, code: &str, message: impl Into<String>) -> Value {
    json!({
        "protocol": PROTOCOL_ID,
        "id": id,
        "ok": false,
        "error": {
            "code": code,
            "message": message.into(),
        },
    })
}

pub(crate) fn node_value(entry: &CatalogueEntry) -> Value {
    json!({
        "node_id": entry.node_id.0,
        "key": format!("{:016x}", entry.fact.key.0),
        "identity": entry.fact.external_identity,
        "kind": entry.fact.kind.as_str(),
        "path": entry.fact.path,
        "name": entry.fact.name,
        "qualified_name": entry.fact.qualified_name,
        "content_id": entry.fact.content_id.map(|id| format!("{:016x}", id.0)),
        "span": entry.fact.span.as_ref().map(span_value),
    })
}

pub(crate) fn relationship_value(relation: &RelationKind, entry: &CatalogueEntry) -> Value {
    json!({
        "relation": relation.as_str(),
        "node": node_value(entry),
    })
}

pub(crate) fn unresolved_value(
    reference: &UnresolvedReferenceFact,
    source_node_id: NodeId,
) -> Value {
    json!({
        "source_node_id": source_node_id.0,
        "source_key": format!("{:016x}", reference.source.0),
        "relation": reference.relation.as_str(),
        "expression": reference.expression,
        "candidate_namespace": reference.candidate_namespace,
        "candidate_name": reference.candidate_name,
        "reason": reference.reason.as_str(),
        "span": reference.span.as_ref().map(span_value),
    })
}

pub(crate) fn span_value(span: &SourceSpan) -> Value {
    json!({
        "path": span.path,
        "start_line": span.start_line,
        "start_column": span.start_column,
        "end_line": span.end_line,
        "end_column": span.end_column,
    })
}

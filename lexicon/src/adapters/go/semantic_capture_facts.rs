use std::collections::BTreeSet;

use crate::{AdapterError, FactRecord, NodeRecord, SourceSpan};

use super::{
    identities,
    protocol_records::{Record, RelationshipKind, Span},
    semantic_facts_support::{EdgeKey, push_edge},
};

pub(super) fn add(
    record: &Record,
    records: &mut Vec<FactRecord>,
    nodes: &mut BTreeSet<String>,
    edges: &mut BTreeSet<EdgeKey>,
) -> Result<bool, AdapterError> {
    let Record::Relationship {
        source,
        target,
        kind: RelationshipKind::References,
        target_name,
        capture_index,
        owner,
        span,
    } = record
    else {
        return Ok(false);
    };

    let source_id = identities::node_id(source)?;
    if !nodes.contains(&source_id) {
        return Err(AdapterError::new(format!(
            "Go semantic capture source is not materialized: {source:?}"
        )));
    }
    let name = target_name
        .as_deref()
        .ok_or_else(|| AdapterError::new("Go semantic capture is missing target_name"))?;
    let index = capture_index
        .ok_or_else(|| AdapterError::new("Go semantic capture is missing capture_index"))?;
    let identity = target
        .clone()
        .unwrap_or_else(|| identities::capture(&source_id, index, name));
    let target_id = identities::node_id_for_kind(&identity, "variable")?;
    let location = span.as_ref().map(|value| source_span(owner, value));
    let node_location = location.as_ref().map(|value| SourceSpan {
        path: value.path.clone(),
        start_line: value.start_line,
        start_column: value.start_column,
        end_line: value.end_line,
        end_column: value.end_column,
    });

    if nodes.insert(target_id.clone()) {
        records.push(FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: None,
            id: target_id.clone(),
            kind: "variable".into(),
            name: name.into(),
            owner: node_location.as_ref().map(|_| owner.clone()),
            path: owner.clone(),
            qualified_name: format!("{owner}::{name}"),
            span: node_location,
        }));
    }
    push_edge(
        records,
        edges,
        source_id,
        target_id,
        "references",
        Some(owner.clone()),
        location,
    );
    Ok(true)
}

fn source_span(owner: &str, span: &Span) -> SourceSpan {
    SourceSpan {
        path: owner.to_owned(),
        start_line: span.start_line,
        start_column: span.start_column,
        end_line: span.end_line,
        end_column: span.end_column,
    }
}

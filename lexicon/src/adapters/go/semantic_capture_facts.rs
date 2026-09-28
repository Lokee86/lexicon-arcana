use crate::{AdapterError, FactRecord, NodeRecord, SourceSpan};

use super::{
    identities,
    observations::{Observation, Span},
    semantic_fact_index::FactIndex,
    semantic_facts_support::push_edge,
};

pub(super) fn add(
    observation: &Observation,
    records: &mut Vec<FactRecord>,
    index: &mut FactIndex,
) -> Result<bool, AdapterError> {
    let Observation::Capture {
        source_key,
        target_key,
        target_name,
        capture_index,
        owner,
        span,
    } = observation
    else {
        return Ok(false);
    };

    let source_id = index.node_id(source_key)?;
    if !index.contains_node(&source_id) {
        return Err(AdapterError::new(format!(
            "Go semantic capture source is not materialized: {source_key:?}"
        )));
    }
    let identity = target_key
        .clone()
        .unwrap_or_else(|| identities::capture(&source_id, *capture_index, target_name));
    let target_id = index.node_id_for_kind(&identity, "variable")?;
    let location = span.as_ref().map(|value| source_span(owner, value));
    let node_location = location.as_ref().map(|value| SourceSpan {
        path: value.path.clone(),
        start_line: value.start_line,
        start_column: value.start_column,
        end_line: value.end_line,
        end_column: value.end_column,
    });

    index.push_node(
        records,
        NodeRecord {
            attributes: None,
            content_id: None,
            id: target_id.clone(),
            kind: "variable".into(),
            name: target_name.clone(),
            owner: node_location.as_ref().map(|_| owner.clone()),
            path: owner.clone(),
            qualified_name: format!("{owner}::{target_name}"),
            span: node_location,
        },
    );
    push_edge(
        records,
        index,
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

use crate::{AdapterError, FactRecord, NodeRecord, SourceSpan};

use super::{
    identities,
    observations::{Observation, Span},
    semantic_fact_index::FactIndex,
    semantic_facts_support::push_edge,
    semantic_policy,
};

pub(crate) fn add(
    observation: &Observation,
    records: &mut Vec<FactRecord>,
    index: &mut FactIndex,
) -> Result<bool, AdapterError> {
    let Observation::Dataflow {
        source_key,
        target_key,
        access,
        owner,
        span,
    } = observation
    else {
        return Ok(false);
    };

    let source_id = index.semantic_node_id(source_key)?;
    let target_identity = index.canonical_semantic_identity(target_key)?;
    let symbol = data_symbol(&target_identity)?;
    let target_id = index.node_id_for_kind(&target_identity, symbol.kind)?;
    index.push_node(
        records,
        NodeRecord {
            attributes: None,
            content_id: None,
            id: target_id.clone(),
            kind: symbol.kind.into(),
            name: symbol.name.clone(),
            owner: Some(symbol.owner.clone()),
            path: symbol.owner.clone(),
            qualified_name: format!("{}::{}", symbol.owner, symbol.name),
            span: Some(SourceSpan {
                path: symbol.owner.clone(),
                start_line: symbol.line,
                start_column: symbol.column,
                end_line: symbol.line,
                end_column: symbol.column.saturating_add(symbol.name.len() as u64),
            }),
        },
    );

    let relation = semantic_policy::dataflow_relation(*access);
    push_edge(
        records,
        index,
        source_id,
        target_id,
        relation,
        Some(owner.clone()),
        Some(source_span(owner, span)),
    );
    Ok(true)
}

struct DataSymbol {
    kind: &'static str,
    owner: String,
    line: u64,
    column: u64,
    name: String,
}

fn data_symbol(identity: &str) -> Result<DataSymbol, AdapterError> {
    let (kind, body) = identity
        .split_once(':')
        .ok_or_else(|| AdapterError::new(format!("invalid Go dataflow target {identity:?}")))?;
    let fact_kind = match kind {
        "variable" => "variable",
        "field" => "field",
        "constant" => "constant",
        _ => {
            return Err(AdapterError::new(format!(
                "Go dataflow target is not a legacy data symbol: {identity:?}"
            )));
        }
    };
    let mut parts = body.rsplitn(5, ':');
    let name = required_part(parts.next(), identity, "name")?;
    let column = parse_position(
        required_part(parts.next(), identity, "column")?,
        identity,
        "column",
    )?;
    let line = parse_position(
        required_part(parts.next(), identity, "line")?,
        identity,
        "line",
    )?;
    let owner = required_part(parts.next(), identity, "owner")?;
    let namespace = required_part(parts.next(), identity, "namespace")?;
    let canonical = identities::positioned_symbol(fact_kind, namespace, owner, line, column, name)?;
    if canonical != identity {
        return Err(AdapterError::new(format!(
            "non-canonical Go dataflow target {identity:?}"
        )));
    }
    Ok(DataSymbol {
        kind: fact_kind,
        owner: owner.to_owned(),
        line,
        column,
        name: name.to_owned(),
    })
}

fn required_part<'a>(
    value: Option<&'a str>,
    identity: &str,
    label: &str,
) -> Result<&'a str, AdapterError> {
    value.filter(|value| !value.is_empty()).ok_or_else(|| {
        AdapterError::new(format!(
            "Go dataflow target {identity:?} is missing {label}"
        ))
    })
}

fn parse_position(value: &str, identity: &str, label: &str) -> Result<u64, AdapterError> {
    let parsed = value.parse::<u64>().map_err(|_| {
        AdapterError::new(format!(
            "Go dataflow target {identity:?} has invalid {label}"
        ))
    })?;
    if parsed == 0 {
        return Err(AdapterError::new(format!(
            "Go dataflow target {identity:?} has zero {label}"
        )));
    }
    Ok(parsed)
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

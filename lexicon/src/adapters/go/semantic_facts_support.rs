use std::collections::{BTreeMap, BTreeSet};

use crate::{AdapterError, EdgeRecord, FactRecord, SourceSpan};

use super::{discovery::Inventory, identities};

pub(super) type EdgeKey = (String, String, String, String);

pub(super) fn container_id(metadata: &BTreeMap<String, String>) -> Result<String, AdapterError> {
    identities::node_id(required(metadata, "container")?)
}

pub(super) fn required<'a>(
    metadata: &'a BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str, AdapterError> {
    metadata
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AdapterError::new(format!("Go semantic declaration is missing {key}")))
}

pub(super) fn parent_id(owner: &str, inventory: &Inventory) -> Result<String, AdapterError> {
    match owner.rsplit_once('/') {
        Some((parent, _)) => identities::node_id(&identities::directory(parent)),
        None => identities::node_id(&identities::repository(&inventory.repository)),
    }
}

pub(super) fn push_edge(
    records: &mut Vec<FactRecord>,
    seen: &mut BTreeSet<EdgeKey>,
    source: String,
    target: String,
    relation: &str,
    owner: Option<String>,
    span: Option<SourceSpan>,
) {
    let span_key = span
        .as_ref()
        .map(|value| {
            format!(
                "{}:{}:{}:{}:{}",
                value.path, value.start_line, value.start_column, value.end_line, value.end_column
            )
        })
        .unwrap_or_default();
    let key = (
        source.clone(),
        target.clone(),
        relation.to_owned(),
        span_key,
    );
    if seen.insert(key) {
        records.push(FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner,
            relation: relation.into(),
            source,
            span,
            target,
        }));
    }
}

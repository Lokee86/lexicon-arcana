use std::collections::BTreeSet;

use crate::{AdapterError, FactRecord};

use super::{
    discovery::Inventory,
    identities,
    protocol_records::{Record, RelationshipKind},
    semantic_facts_support::{EdgeKey, ensure_relationship_target, push_edge},
};

pub(super) fn add(
    record: &Record,
    inventory: &Inventory,
    records: &mut Vec<FactRecord>,
    nodes: &mut BTreeSet<String>,
    edges: &mut BTreeSet<EdgeKey>,
) -> Result<bool, AdapterError> {
    let Record::Relationship {
        source,
        target,
        kind,
        owner,
        ..
    } = record
    else {
        return Ok(false);
    };

    let source_id = identities::node_id(source)?;
    if !nodes.contains(&source_id) {
        return Err(AdapterError::new(format!(
            "Go semantic relationship source is not materialized: {source:?}"
        )));
    }
    let target_id = ensure_relationship_target(target, inventory, records, nodes, edges)?;
    if matches!(kind, RelationshipKind::Implements) && source_id == target_id {
        return Err(AdapterError::new(
            "Go semantic relationship attempted an implements self-edge",
        ));
    }
    push_edge(
        records,
        edges,
        source_id,
        target_id,
        relationship_name(*kind),
        Some(owner.clone()),
        None,
    );
    Ok(true)
}

fn relationship_name(kind: RelationshipKind) -> &'static str {
    match kind {
        RelationshipKind::Implements => "implements",
        RelationshipKind::Extends => "extends",
        RelationshipKind::Overrides => "overrides",
        RelationshipKind::References => "references",
    }
}

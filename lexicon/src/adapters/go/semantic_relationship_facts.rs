use crate::{AdapterError, FactRecord};

use super::{
    discovery::Inventory,
    protocol_records::{Record, RelationshipKind},
    semantic_fact_index::FactIndex,
    semantic_facts_support::{ensure_relationship_target, push_edge},
};

pub(super) fn add(
    record: &Record,
    inventory: &Inventory,
    records: &mut Vec<FactRecord>,
    index: &mut FactIndex,
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

    let source_id = index.node_id(source)?;
    if !index.contains_node(&source_id) {
        return Err(AdapterError::new(format!(
            "Go semantic relationship source is not materialized: {source:?}"
        )));
    }
    let target = target
        .as_deref()
        .ok_or_else(|| AdapterError::new("Go semantic relationship target is missing"))?;
    let target_id = ensure_relationship_target(target, inventory, records, index)?;
    if matches!(kind, RelationshipKind::Implements) && source_id == target_id {
        return Err(AdapterError::new(
            "Go semantic relationship attempted an implements self-edge",
        ));
    }
    let edge_owner = index.node_owner(&source_id).or_else(|| Some(owner.clone()));
    push_edge(
        records,
        index,
        source_id,
        target_id,
        relationship_name(*kind),
        edge_owner,
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

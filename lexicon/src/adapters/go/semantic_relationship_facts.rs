use crate::{AdapterError, FactRecord};

use super::{
    discovery::Inventory,
    observations::{Observation, RelationshipKind},
    semantic_fact_index::FactIndex,
    semantic_facts_support::{ensure_relationship_target, push_edge},
    semantic_policy,
};

pub(super) fn add(
    observation: &Observation,
    inventory: &Inventory,
    records: &mut Vec<FactRecord>,
    index: &mut FactIndex,
) -> Result<bool, AdapterError> {
    let Observation::Relationship {
        source_key,
        target_key,
        kind,
        owner,
        ..
    } = observation
    else {
        return Ok(false);
    };

    let source_id = index.semantic_node_id(source_key)?;
    if !index.contains_node(&source_id) {
        return Err(AdapterError::new(format!(
            "Go semantic relationship source is not materialized: {source_key:?}"
        )));
    }
    let target_id = ensure_relationship_target(target_key, inventory, records, index)?;
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
        semantic_policy::relationship_relation(*kind),
        edge_owner,
        None,
    );
    Ok(true)
}

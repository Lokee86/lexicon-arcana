mod load;
mod model;
mod query;

use std::collections::BTreeMap;

use crate::{EdgeRecord, UnresolvedRecord};

pub use model::{
    LookupDirection, LookupEdge, LookupError, LookupNode, LookupReference, LookupUnresolved,
};

#[derive(Debug, Clone)]
pub struct SnapshotLookup {
    snapshot_id: String,
    nodes: BTreeMap<String, LookupNode>,
    edges: Vec<StoredEdge>,
    unresolved: Vec<StoredUnresolved>,
    relationships_loaded: bool,
}

#[derive(Debug, Clone)]
struct StoredEdge {
    language: String,
    record: EdgeRecord,
}

#[derive(Debug, Clone)]
struct StoredUnresolved {
    language: String,
    record: UnresolvedRecord,
}

impl SnapshotLookup {
    fn validate_sources(&self) -> Result<(), LookupError> {
        for edge in &self.edges {
            if !self.nodes.contains_key(&edge.record.source) {
                return Err(crate::StorageError::Operation(format!(
                    "lookup edge has unknown source {}",
                    edge.record.source
                ))
                .into());
            }
        }
        for unresolved in &self.unresolved {
            if !self.nodes.contains_key(&unresolved.record.source) {
                return Err(crate::StorageError::Operation(format!(
                    "lookup unresolved record has unknown source {}",
                    unresolved.record.source
                ))
                .into());
            }
        }
        Ok(())
    }

    fn sort_evidence(&mut self) {
        self.edges.sort_by(|left, right| {
            left.record
                .source
                .cmp(&right.record.source)
                .then_with(|| left.record.target.cmp(&right.record.target))
                .then_with(|| left.record.relation.cmp(&right.record.relation))
                .then_with(|| left.language.cmp(&right.language))
        });
        self.unresolved.sort_by(|left, right| {
            left.record
                .source
                .cmp(&right.record.source)
                .then_with(|| left.record.relation.cmp(&right.record.relation))
                .then_with(|| left.record.expression.cmp(&right.record.expression))
                .then_with(|| left.language.cmp(&right.language))
        });
    }
}

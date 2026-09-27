use std::collections::BTreeMap;

use super::LexiconSnapshotError;
use super::object::{FactRecord, NodeRecord, RecordCounts};
use super::records::{
    CompactNodeIds, CompatibilityCounts, ExternalNodeIds, convert_edge, convert_node,
    convert_unresolved, finish_repository_facts, insert_node_record,
};
use crate::repository::RepositoryFacts;

/// First pass over Lexicon objects. Only node records survive object decoding.
pub(super) struct NodePass {
    nodes: BTreeMap<String, NodeRecord>,
    counts: RecordCounts,
    conflict: Option<LexiconSnapshotError>,
}

impl NodePass {
    pub(super) fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            counts: RecordCounts::default(),
            conflict: None,
        }
    }

    pub(super) fn ingest(&mut self, records: Vec<FactRecord>, counts: RecordCounts) {
        if self.conflict.is_some() {
            return;
        }
        let Some(total) = self.counts.checked_add(counts) else {
            self.conflict = Some(LexiconSnapshotError::Malformed("record count overflow"));
            return;
        };
        self.counts = total;

        for record in records {
            let FactRecord::Node(record) = record else {
                continue;
            };
            if let Err(error) = insert_node_record(&mut self.nodes, record) {
                self.conflict = Some(error);
                return;
            }
        }
    }

    pub(super) fn finish(self) -> Result<RelationPass, LexiconSnapshotError> {
        if let Some(error) = self.conflict {
            return Err(error);
        }

        let mut facts = RepositoryFacts {
            nodes: Vec::with_capacity(self.nodes.len()),
            edges: Vec::with_capacity(self.counts.edges),
            unresolved: Vec::with_capacity(self.counts.unresolved),
        };
        let mut external_ids = ExternalNodeIds::new();
        let mut compact_ids = CompactNodeIds::new();
        let mut compatibility = CompatibilityCounts::new();
        for record in self.nodes.into_values() {
            facts.nodes.push(convert_node(
                record,
                &mut external_ids,
                &mut compact_ids,
                &mut compatibility,
            )?);
        }
        Ok(RelationPass {
            facts,
            external_ids,
            compatibility,
            edge_error: None,
            unresolved_error: None,
        })
    }
}

/// Second pass over Lexicon objects. Nodes are never materialized; edges and
/// unresolved references are converted into their final Arcana form.
pub(super) struct RelationPass {
    facts: RepositoryFacts,
    external_ids: ExternalNodeIds,
    compatibility: CompatibilityCounts,
    edge_error: Option<LexiconSnapshotError>,
    unresolved_error: Option<LexiconSnapshotError>,
}

impl RelationPass {
    pub(super) fn ingest(&mut self, records: Vec<FactRecord>) {
        for record in records {
            match record {
                FactRecord::Node(_) => {}
                FactRecord::Edge(record) if self.edge_error.is_none() => {
                    match convert_edge(&self.external_ids, record, &mut self.compatibility) {
                        Ok(Some(edge)) => self.facts.edges.push(edge),
                        Ok(None) => {}
                        Err(error) => self.edge_error = Some(error),
                    }
                }
                FactRecord::Edge(_) => {}
                FactRecord::Unresolved(record) if self.unresolved_error.is_none() => {
                    match convert_unresolved(&self.external_ids, record, &mut self.compatibility) {
                        Ok(Some(reference)) => self.facts.unresolved.push(reference),
                        Ok(None) => {}
                        Err(error) => self.unresolved_error = Some(error),
                    }
                }
                FactRecord::Unresolved(_) => {}
            }
        }
    }

    pub(super) fn finish(self) -> Result<(RepositoryFacts, Vec<String>), LexiconSnapshotError> {
        if let Some(error) = self.edge_error {
            return Err(error);
        }
        if let Some(error) = self.unresolved_error {
            return Err(error);
        }
        finish_repository_facts(self.facts, self.compatibility)
    }
}

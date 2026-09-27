use crate::repository::{NodeKey, RepositoryFacts};
use crate::synthetic::NodeId;

use super::build_indexes::{sorted_dense_ids, sorted_kind_index};
use super::build_ownership::build_ownership;
use super::canonical::Contribution;
use super::{
    CompactEdgeRecord, CompactNodeRecord, CompactStringTable, CompactUnresolvedRecord,
    RepositoryStoreWriteError, StringId, StringTableBuilder,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompactOwnershipRecord {
    pub path: StringId,
    pub contribution_start: u64,
    pub contribution_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CompactKindIndexRecord {
    pub kind_code: u16,
    pub node_id: NodeId,
}

/// Canonical compact staging state for repository builds.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CompactRepositoryBuild {
    pub strings: CompactStringTable,
    pub nodes: Vec<CompactNodeRecord>,
    pub edges: Vec<CompactEdgeRecord>,
    pub unresolved: Vec<CompactUnresolvedRecord>,
    pub ownership: Vec<CompactOwnershipRecord>,
    pub contributions: Vec<Contribution>,
    pub name_index: Vec<NodeId>,
    pub path_index: Vec<NodeId>,
    pub kind_index: Vec<CompactKindIndexRecord>,
}

impl CompactRepositoryBuild {
    pub(crate) fn from_facts(facts: &RepositoryFacts) -> Result<Self, RepositoryStoreWriteError> {
        let canonical = super::canonical::CanonicalFacts::prepare(facts)?;
        canonical.node_count()?;

        let mut string_builder = StringTableBuilder::default();
        string_builder.collect_facts(facts);
        for path in canonical.ownership.keys() {
            string_builder.insert_ref(path);
        }
        let strings = string_builder.finish()?;

        let nodes = canonical
            .nodes
            .iter()
            .map(|node| CompactNodeRecord::from_fact(node.fact, &strings, node.occurrence_count))
            .collect::<Result<Vec<_>, _>>()?;
        let edges = canonical
            .edges
            .iter()
            .map(|edge| CompactEdgeRecord::from_fact(edge, &strings))
            .collect::<Result<Vec<_>, _>>()?;
        let unresolved = canonical
            .unresolved
            .iter()
            .map(|reference| CompactUnresolvedRecord::from_fact(reference, &strings))
            .collect::<Result<Vec<_>, _>>()?;

        Self::from_canonical_records(strings, nodes, edges, unresolved)
    }

    pub(super) fn from_canonical_records(
        strings: CompactStringTable,
        nodes: Vec<CompactNodeRecord>,
        edges: Vec<CompactEdgeRecord>,
        unresolved: Vec<CompactUnresolvedRecord>,
    ) -> Result<Self, RepositoryStoreWriteError> {
        validate_references(&nodes, &edges, &unresolved)?;
        let name_index = sorted_dense_ids(&nodes, |record| record.name)?;
        let path_index = sorted_dense_ids(&nodes, |record| record.path)?;
        let kind_index = sorted_kind_index(&nodes)?;

        let mut build = Self {
            strings,
            nodes,
            edges,
            unresolved,
            ownership: Vec::new(),
            contributions: Vec::new(),
            name_index,
            path_index,
            kind_index,
        };
        (build.ownership, build.contributions) = build_ownership(&build)?;
        Ok(build)
    }

    pub(crate) fn node_id(&self, key: NodeKey) -> Option<NodeId> {
        self.nodes
            .binary_search_by_key(&key, |node| node.key)
            .ok()
            .and_then(|index| u32::try_from(index).ok())
            .map(NodeId)
    }
}

fn validate_references(
    nodes: &[CompactNodeRecord],
    edges: &[CompactEdgeRecord],
    unresolved: &[CompactUnresolvedRecord],
) -> Result<(), RepositoryStoreWriteError> {
    for edge in edges {
        for key in [edge.source, edge.target] {
            if !contains_node(nodes, key) {
                return Err(RepositoryStoreWriteError::MissingEdgeEndpoint { key });
            }
        }
    }
    for reference in unresolved {
        if !contains_node(nodes, reference.source) {
            return Err(RepositoryStoreWriteError::MissingUnresolvedSource {
                key: reference.source,
            });
        }
    }
    Ok(())
}

fn contains_node(nodes: &[CompactNodeRecord], key: NodeKey) -> bool {
    nodes.binary_search_by_key(&key, |node| node.key).is_ok()
}

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use crate::repository::{NodeKey, NodeKind, RepositoryFacts};
use crate::synthetic::NodeId;

use super::build_indexes::{sorted_dense_ids, sorted_kind_index};
use super::build_ownership::{build_ownership, compact_node_owner};
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
#[doc(hidden)]
pub struct CompactRepositoryBuild {
    pub(crate) strings: CompactStringTable,
    pub(crate) nodes: Vec<CompactNodeRecord>,
    pub(crate) edges: Vec<CompactEdgeRecord>,
    pub(crate) unresolved: Vec<CompactUnresolvedRecord>,
    pub(crate) ownership: Vec<CompactOwnershipRecord>,
    pub(crate) contributions: Vec<Contribution>,
    pub(crate) name_index: Vec<NodeId>,
    pub(crate) path_index: Vec<NodeId>,
    pub(crate) kind_index: Vec<CompactKindIndexRecord>,
}

/// Canonical compact records for a selected repository path set.
#[derive(Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct CompactRepositoryDelta {
    pub(crate) strings: CompactStringTable,
    pub(crate) nodes: Vec<CompactNodeRecord>,
    pub(crate) edges: Vec<CompactEdgeRecord>,
    pub(crate) unresolved: Vec<CompactUnresolvedRecord>,
    owned_nodes: BTreeMap<StringId, Vec<NodeKey>>,
}

impl CompactRepositoryDelta {
    pub(super) fn from_canonical_records(
        strings: CompactStringTable,
        nodes: Vec<CompactNodeRecord>,
        edges: Vec<CompactEdgeRecord>,
        unresolved: Vec<CompactUnresolvedRecord>,
    ) -> Result<Self, RepositoryStoreWriteError> {
        let mut owned_nodes = BTreeMap::<StringId, Vec<NodeKey>>::new();
        for node in &nodes {
            if let Some(path) = compact_node_owner(&strings, node)? {
                owned_nodes.entry(path).or_default().push(node.key);
            }
        }
        for keys in owned_nodes.values_mut() {
            keys.sort_unstable();
            keys.dedup();
        }
        Ok(Self {
            strings,
            nodes,
            edges,
            unresolved,
            owned_nodes,
        })
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub(crate) fn edges(&self) -> &[CompactEdgeRecord] {
        &self.edges
    }

    pub fn unresolved_count(&self) -> usize {
        self.unresolved.len()
    }

    pub fn owned_node_keys(&self, paths: &[String]) -> Vec<NodeKey> {
        let mut keys = BTreeSet::new();
        for path in paths {
            let Ok(path_id) = self.strings.id(path) else {
                continue;
            };
            if let Some(owned) = self.owned_nodes.get(&path_id) {
                keys.extend(owned.iter().copied());
            }
        }
        keys.into_iter().collect()
    }
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

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn unresolved_count(&self) -> usize {
        self.unresolved.len()
    }

    pub fn string_count(&self) -> usize {
        self.strings.len()
    }

    pub fn repository_identity(&self, store_checksum: u64) -> u64 {
        let repository_kind = super::format::node_kind_code(&NodeKind::Repository);
        let mut repositories = self
            .nodes
            .iter()
            .filter(|node| node.kind_code == repository_kind);
        match (repositories.next(), repositories.next()) {
            (Some(repository), None) => repository.key.as_u64(),
            _ => store_checksum,
        }
    }

    pub fn owned_node_keys(&self, paths: &[String]) -> Vec<NodeKey> {
        let mut keys = BTreeSet::new();
        for contribution in self.owned_contributions(paths) {
            if contribution.kind == super::canonical::ContributionKind::Node
                && let Some(node) = self.nodes.get(contribution.record_index as usize)
            {
                keys.insert(node.key);
            }
        }
        keys.into_iter().collect()
    }

    pub(crate) fn owned_edges(&self, paths: &[String]) -> Vec<CompactEdgeRecord> {
        let mut indexes = BTreeSet::new();
        for contribution in self.owned_contributions(paths) {
            if contribution.kind == super::canonical::ContributionKind::Edge {
                indexes.insert(contribution.record_index);
            }
        }
        indexes
            .into_iter()
            .filter_map(|index| self.edges.get(index as usize).copied())
            .collect()
    }

    fn owned_contributions<'a>(
        &'a self,
        paths: &'a [String],
    ) -> impl Iterator<Item = Contribution> + 'a {
        paths
            .iter()
            .flat_map(|path| {
                let Ok(path_id) = self.strings.id(path) else {
                    return &[][..];
                };
                let Ok(owner_index) = self
                    .ownership
                    .binary_search_by_key(&path_id, |record| record.path)
                else {
                    return &[][..];
                };
                let owner = self.ownership[owner_index];
                let start = owner.contribution_start as usize;
                let end = start + owner.contribution_count as usize;
                &self.contributions[start..end]
            })
            .copied()
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

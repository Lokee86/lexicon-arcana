use std::collections::HashMap;

use crate::repository::{NodeKey, RepositoryFacts};
use crate::synthetic::NodeId;

use super::canonical::{CanonicalFacts, Contribution};
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

/// Canonical compact staging state for managed repository builds.
///
/// This deliberately owns no per-fact strings. Text lives once in the string
/// table; records refer to it by StringId, and node references use compact keys.
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
    node_ids: HashMap<NodeKey, NodeId>,
}

impl CompactRepositoryBuild {
    /// Compatibility constructor used until direct Lexicon streaming lands.
    ///
    /// Later managed-sync phases populate this shape without first creating
    /// RepositoryFacts. This conversion gives that path a semantic oracle
    /// without changing existing public APIs.
    pub(crate) fn from_facts(facts: &RepositoryFacts) -> Result<Self, RepositoryStoreWriteError> {
        let canonical = CanonicalFacts::prepare(facts)?;
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

        let node_ids = dense_node_ids(&nodes)?;
        let (ownership, contributions) = compact_ownership(&canonical, &strings)?;
        let name_index = sorted_dense_ids(&nodes, |record| record.name)?;
        let path_index = sorted_dense_ids(&nodes, |record| record.path)?;
        let kind_index = sorted_kind_index(&nodes)?;

        Ok(Self {
            strings,
            nodes,
            edges,
            unresolved,
            ownership,
            contributions,
            name_index,
            path_index,
            kind_index,
            node_ids,
        })
    }

    pub(crate) fn node_id(&self, key: NodeKey) -> Option<NodeId> {
        self.node_ids.get(&key).copied()
    }
}

fn dense_node_ids(
    nodes: &[CompactNodeRecord],
) -> Result<HashMap<NodeKey, NodeId>, RepositoryStoreWriteError> {
    let mut ids = HashMap::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        let id = u32::try_from(index)
            .map(NodeId)
            .map_err(|_| RepositoryStoreWriteError::TooManyNodes)?;
        ids.insert(node.key, id);
    }
    Ok(ids)
}

fn compact_ownership(
    canonical: &CanonicalFacts<'_>,
    strings: &CompactStringTable,
) -> Result<(Vec<CompactOwnershipRecord>, Vec<Contribution>), RepositoryStoreWriteError> {
    let mut ownership = Vec::with_capacity(canonical.ownership.len());
    let total = canonical
        .ownership
        .values()
        .try_fold(0_usize, |total, values| total.checked_add(values.len()))
        .ok_or(RepositoryStoreWriteError::TooManyContributions)?;
    let mut contributions = Vec::with_capacity(total);

    for (path, values) in &canonical.ownership {
        let contribution_start = u64::try_from(contributions.len())
            .map_err(|_| RepositoryStoreWriteError::TooManyContributions)?;
        let contribution_count = u64::try_from(values.len())
            .map_err(|_| RepositoryStoreWriteError::TooManyContributions)?;
        ownership.push(CompactOwnershipRecord {
            path: strings.id(path)?,
            contribution_start,
            contribution_count,
        });
        contributions.extend(values.iter().copied());
    }
    Ok((ownership, contributions))
}

fn sorted_dense_ids(
    nodes: &[CompactNodeRecord],
    key: impl Fn(&CompactNodeRecord) -> StringId,
) -> Result<Vec<NodeId>, RepositoryStoreWriteError> {
    let mut ids = (0..nodes.len())
        .map(|index| {
            u32::try_from(index)
                .map(NodeId)
                .map_err(|_| RepositoryStoreWriteError::TooManyNodes)
        })
        .collect::<Result<Vec<_>, _>>()?;
    ids.sort_unstable_by_key(|id| (key(&nodes[id.0 as usize]), *id));
    Ok(ids)
}

fn sorted_kind_index(
    nodes: &[CompactNodeRecord],
) -> Result<Vec<CompactKindIndexRecord>, RepositoryStoreWriteError> {
    let mut records = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            Ok(CompactKindIndexRecord {
                kind_code: node.kind_code,
                node_id: u32::try_from(index)
                    .map(NodeId)
                    .map_err(|_| RepositoryStoreWriteError::TooManyNodes)?,
            })
        })
        .collect::<Result<Vec<_>, RepositoryStoreWriteError>>()?;
    records.sort_unstable_by_key(|record| (record.kind_code, record.node_id));
    Ok(records)
}

#[cfg(test)]
mod tests;

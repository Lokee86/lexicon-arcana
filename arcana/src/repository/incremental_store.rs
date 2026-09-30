use std::collections::BTreeSet;

use crate::repository_store::{
    CompactRepositoryBuild, CompactRepositoryDelta, RepositoryStoreFile,
};
use crate::snapshot::OverlayChanges;
use crate::synthetic::GraphDataset;

use super::incremental_diff::{edge_difference, key_difference};
use super::{
    CompiledRepositoryGraph, FactOwnershipError, IncrementalError, IncrementalUpdate, NodeKey,
    RepositoryFacts, compile_compact_repository_graph, compile_repository_graph, node_owner,
    normalize_repository_path,
};

pub struct VerifiedSnapshotUpdatePlan {
    graph: CompiledRepositoryGraph,
    changes: OverlayChanges,
    changed_file_count: usize,
}

impl VerifiedSnapshotUpdatePlan {
    pub fn finish(self, facts: RepositoryFacts) -> IncrementalUpdate {
        IncrementalUpdate::new(facts, self.graph, self.changes, self.changed_file_count)
    }
}

#[doc(hidden)]
pub struct VerifiedCompactSnapshotUpdatePlan {
    graph: CompiledRepositoryGraph,
    changes: OverlayChanges,
    changed_file_count: usize,
}

#[doc(hidden)]
pub struct CompactIncrementalUpdate {
    pub repository: CompactRepositoryBuild,
    pub graph: CompiledRepositoryGraph,
    pub changes: OverlayChanges,
    changed_file_count: usize,
}

impl CompactIncrementalUpdate {
    pub const fn changed_file_count(&self) -> usize {
        self.changed_file_count
    }
}

impl VerifiedCompactSnapshotUpdatePlan {
    pub fn finish(self, repository: CompactRepositoryBuild) -> CompactIncrementalUpdate {
        CompactIncrementalUpdate {
            repository,
            graph: self.graph,
            changes: self.changes,
            changed_file_count: self.changed_file_count,
        }
    }
}

pub fn plan_verified_snapshot_update_from_store(
    base_store: &mut RepositoryStoreFile,
    current_facts: &RepositoryFacts,
    changed_paths: &[String],
    packed_base: &GraphDataset,
) -> Result<VerifiedSnapshotUpdatePlan, IncrementalError> {
    let changed_paths = normalized_paths(changed_paths)?;
    let base_changed = base_store.owned_node_keys(&changed_paths)?;
    let current_changed = changed_node_keys(current_facts, &changed_paths)?;
    verify_node_set(&base_changed, &current_changed)?;

    let graph = compile_repository_graph(current_facts)?;
    verify_base_node_count(packed_base, &graph)?;
    let changes = edge_difference(&packed_base.edges, &graph.dataset.edges);
    Ok(VerifiedSnapshotUpdatePlan {
        graph,
        changes,
        changed_file_count: changed_paths.len(),
    })
}

#[doc(hidden)]
pub fn verify_compact_delta_node_set_from_store(
    base_store: &mut RepositoryStoreFile,
    delta: &CompactRepositoryDelta,
    changed_paths: &[String],
) -> Result<(), IncrementalError> {
    let changed_paths = normalized_paths(changed_paths)?;
    let base_changed = base_store.owned_node_keys(&changed_paths)?;
    let current_changed = delta.owned_node_keys(&changed_paths);
    verify_node_set(&base_changed, &current_changed)
}

#[doc(hidden)]
pub fn plan_verified_compact_snapshot_update_from_store(
    base_store: &mut RepositoryStoreFile,
    current: &CompactRepositoryBuild,
    changed_paths: &[String],
    packed_base: &GraphDataset,
) -> Result<VerifiedCompactSnapshotUpdatePlan, IncrementalError> {
    let changed_paths = normalized_paths(changed_paths)?;
    let base_changed = base_store.owned_node_keys(&changed_paths)?;
    let current_changed = current.owned_node_keys(&changed_paths);
    verify_node_set(&base_changed, &current_changed)?;

    let graph = compile_compact_repository_graph(current)?;
    verify_base_node_count(packed_base, &graph)?;
    let changes = edge_difference(&packed_base.edges, &graph.dataset.edges);
    Ok(VerifiedCompactSnapshotUpdatePlan {
        graph,
        changes,
        changed_file_count: changed_paths.len(),
    })
}

fn verify_node_set(base: &[NodeKey], current: &[NodeKey]) -> Result<(), IncrementalError> {
    if base == current {
        return Ok(());
    }
    let (added, removed) = key_difference(base, current);
    Err(IncrementalError::NodeSetChanged { added, removed })
}

fn verify_base_node_count(
    packed_base: &GraphDataset,
    graph: &CompiledRepositoryGraph,
) -> Result<(), IncrementalError> {
    if packed_base.node_count == graph.dataset.node_count {
        Ok(())
    } else {
        Err(IncrementalError::BaseNodeCountMismatch {
            expected: graph.dataset.node_count,
            actual: packed_base.node_count,
        })
    }
}

fn normalized_paths(paths: &[String]) -> Result<Vec<String>, FactOwnershipError> {
    paths
        .iter()
        .map(|path| normalize_repository_path(path).map_err(FactOwnershipError::InvalidPath))
        .collect()
}

fn changed_node_keys(
    facts: &RepositoryFacts,
    changed_paths: &[String],
) -> Result<Vec<NodeKey>, FactOwnershipError> {
    let changed = changed_paths.iter().cloned().collect::<BTreeSet<_>>();
    let mut keys = BTreeSet::new();
    for node in &facts.nodes {
        if node_owner(node)?.is_some_and(|path| changed.contains(&path)) {
            keys.insert(node.key);
        }
    }
    Ok(keys.into_iter().collect())
}

use std::collections::BTreeSet;

use crate::repository_store::RepositoryStoreFile;
use crate::synthetic::GraphDataset;

use super::incremental_diff::{edge_difference, key_difference};
use super::{
    FactOwnershipError, IncrementalError, IncrementalUpdate, NodeKey, RepositoryFacts,
    compile_repository_graph, node_owner, normalize_repository_path,
};

pub struct VerifiedSnapshotUpdatePlan {
    graph: super::CompiledRepositoryGraph,
    changes: crate::snapshot::OverlayChanges,
    changed_file_count: usize,
}

impl VerifiedSnapshotUpdatePlan {
    pub fn finish(self, facts: RepositoryFacts) -> IncrementalUpdate {
        IncrementalUpdate::new(facts, self.graph, self.changes, self.changed_file_count)
    }
}

pub fn plan_verified_snapshot_update_from_store(
    base_store: &mut RepositoryStoreFile,
    current_facts: &RepositoryFacts,
    changed_paths: &[String],
    packed_base: &GraphDataset,
) -> Result<VerifiedSnapshotUpdatePlan, IncrementalError> {
    let base_changed = base_store.owned_node_keys(changed_paths)?;
    let current_changed = changed_node_keys(&current_facts, changed_paths)?;

    if base_changed != current_changed {
        let (added, removed) = key_difference(&base_changed, &current_changed);
        return Err(IncrementalError::NodeSetChanged { added, removed });
    }

    let graph = compile_repository_graph(&current_facts)?;
    if packed_base.node_count != graph.dataset.node_count {
        return Err(IncrementalError::BaseNodeCountMismatch {
            expected: graph.dataset.node_count,
            actual: packed_base.node_count,
        });
    }

    let changes = edge_difference(&packed_base.edges, &graph.dataset.edges);
    Ok(VerifiedSnapshotUpdatePlan {
        graph,
        changes,
        changed_file_count: changed_paths.len(),
    })
}

fn changed_node_keys(
    facts: &RepositoryFacts,
    changed_paths: &[String],
) -> Result<Vec<NodeKey>, FactOwnershipError> {
    let changed = changed_paths
        .iter()
        .map(|path| normalize_repository_path(path).map_err(FactOwnershipError::InvalidPath))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut keys = BTreeSet::new();
    for node in &facts.nodes {
        if node_owner(node)?.is_some_and(|path| changed.contains(&path)) {
            keys.insert(node.key);
        }
    }
    Ok(keys.into_iter().collect())
}

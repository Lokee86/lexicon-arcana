use std::fmt;

use crate::repository_store::RepositoryStoreReadError;
use crate::snapshot::OverlayChanges;
use crate::synthetic::GraphDataset;

use super::incremental_diff::{edge_difference, key_difference};

use super::{
    CompiledRepositoryGraph, FactOwnershipError, NodeKey, RepositoryCompileError, RepositoryFacts,
    compile_repository_graph, replace_changed_files_owned_base,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncrementalUpdate {
    pub facts: RepositoryFacts,
    pub graph: CompiledRepositoryGraph,
    pub changes: OverlayChanges,
    changed_file_count: usize,
}

impl IncrementalUpdate {
    pub const fn changed_file_count(&self) -> usize {
        self.changed_file_count
    }

    pub(crate) fn new(
        facts: RepositoryFacts,
        graph: CompiledRepositoryGraph,
        changes: OverlayChanges,
        changed_file_count: usize,
    ) -> Self {
        Self {
            facts,
            graph,
            changes,
            changed_file_count,
        }
    }
}

#[derive(Debug)]
pub enum IncrementalError {
    Ownership(FactOwnershipError),
    Store(RepositoryStoreReadError),
    Compile(RepositoryCompileError),
    NodeSetChanged {
        added: Vec<NodeKey>,
        removed: Vec<NodeKey>,
    },
    BaseNodeCountMismatch {
        expected: u32,
        actual: u32,
    },
}

impl fmt::Display for IncrementalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ownership(error) => error.fmt(formatter),
            Self::Store(error) => error.fmt(formatter),
            Self::Compile(error) => error.fmt(formatter),
            Self::NodeSetChanged { added, removed } => write!(
                formatter,
                "incremental graph update requires rebuild: {} node(s) added, {} removed",
                added.len(),
                removed.len()
            ),
            Self::BaseNodeCountMismatch { expected, actual } => write!(
                formatter,
                "packed base has {actual} nodes but updated repository requires {expected}"
            ),
        }
    }
}

impl std::error::Error for IncrementalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Ownership(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::Compile(error) => Some(error),
            _ => None,
        }
    }
}

impl From<FactOwnershipError> for IncrementalError {
    fn from(error: FactOwnershipError) -> Self {
        Self::Ownership(error)
    }
}

impl From<RepositoryStoreReadError> for IncrementalError {
    fn from(error: RepositoryStoreReadError) -> Self {
        Self::Store(error)
    }
}

impl From<RepositoryCompileError> for IncrementalError {
    fn from(error: RepositoryCompileError) -> Self {
        Self::Compile(error)
    }
}

pub fn plan_file_update(
    current_facts: &RepositoryFacts,
    replacement_facts: &RepositoryFacts,
    changed_paths: &[String],
    packed_base: &GraphDataset,
) -> Result<IncrementalUpdate, IncrementalError> {
    plan_file_update_from_verified_base(
        current_facts.clone(),
        replacement_facts,
        changed_paths,
        packed_base,
    )
}

/// Plans an update from an already verified prior snapshot while transferring
/// ownership of unchanged prior facts into the merged result.
pub fn plan_file_update_from_verified_base(
    current_facts: RepositoryFacts,
    replacement_facts: &RepositoryFacts,
    changed_paths: &[String],
    packed_base: &GraphDataset,
) -> Result<IncrementalUpdate, IncrementalError> {
    let current_keys = node_keys(&current_facts);
    let facts = replace_changed_files_owned_base(current_facts, replacement_facts, changed_paths)?;
    let graph = compile_repository_graph(&facts)?;
    let updated_keys = graph.node_keys.clone();

    if current_keys != updated_keys {
        let (added, removed) = key_difference(&current_keys, &updated_keys);
        return Err(IncrementalError::NodeSetChanged { added, removed });
    }
    if packed_base.node_count != graph.dataset.node_count {
        return Err(IncrementalError::BaseNodeCountMismatch {
            expected: graph.dataset.node_count,
            actual: packed_base.node_count,
        });
    }

    let changes = edge_difference(&packed_base.edges, &graph.dataset.edges);
    Ok(IncrementalUpdate::new(
        facts,
        graph,
        changes,
        changed_paths.len(),
    ))
}

fn node_keys(facts: &RepositoryFacts) -> Vec<NodeKey> {
    let mut keys = facts.nodes.iter().map(|node| node.key).collect::<Vec<_>>();
    keys.sort_unstable();
    keys.dedup();
    keys
}

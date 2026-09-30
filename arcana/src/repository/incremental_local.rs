use crate::repository_store::{CompactEdgeRecord, CompactRepositoryDelta, RepositoryStoreFile};
use crate::snapshot::OverlayChanges;
use crate::synthetic::{Edge, EdgeKind};

use super::incremental_diff::{edge_difference, key_difference};
use super::{
    EdgeFact, FactOwnershipError, IncrementalError, NodeKey, RepositoryCompileError,
    normalize_repository_path, relation_to_edge_kind,
};

#[doc(hidden)]
pub fn plan_compact_delta_edge_changes_from_store(
    base_store: &mut RepositoryStoreFile,
    delta: &CompactRepositoryDelta,
    changed_paths: &[String],
) -> Result<OverlayChanges, IncrementalError> {
    let changed_paths = changed_paths
        .iter()
        .map(|path| normalize_repository_path(path).map_err(FactOwnershipError::InvalidPath))
        .collect::<Result<Vec<_>, _>>()?;

    let base_changed = base_store.owned_node_keys(&changed_paths)?;
    let current_changed = delta.owned_node_keys(&changed_paths);
    verify_node_set(&base_changed, &current_changed)?;

    let base_facts = base_store.owned_facts(&changed_paths)?;
    let base_edges = compile_fact_edges(base_store, &base_facts.edges)?;
    let current_edges = compile_compact_edges(base_store, delta.edges())?;
    Ok(edge_difference(&base_edges, &current_edges))
}

fn verify_node_set(base: &[NodeKey], current: &[NodeKey]) -> Result<(), IncrementalError> {
    if base == current {
        return Ok(());
    }
    let (added, removed) = key_difference(base, current);
    Err(IncrementalError::NodeSetChanged { added, removed })
}

fn compile_fact_edges(
    base_store: &mut RepositoryStoreFile,
    edges: &[EdgeFact],
) -> Result<Vec<Edge>, IncrementalError> {
    compile_edges(
        base_store,
        edges.iter().map(|edge| {
            (
                edge.source,
                edge.target,
                relation_to_edge_kind(&edge.relation),
            )
        }),
    )
}

fn compile_compact_edges(
    base_store: &mut RepositoryStoreFile,
    edges: &[CompactEdgeRecord],
) -> Result<Vec<Edge>, IncrementalError> {
    compile_edges(
        base_store,
        edges
            .iter()
            .map(|edge| (edge.source, edge.target, EdgeKind(edge.relation_code))),
    )
}

fn compile_edges(
    base_store: &mut RepositoryStoreFile,
    edges: impl Iterator<Item = (NodeKey, NodeKey, EdgeKind)>,
) -> Result<Vec<Edge>, IncrementalError> {
    let mut graph_edges = Vec::new();
    for (source, target, kind) in edges {
        graph_edges.push(Edge {
            source: node_id(base_store, source)?,
            target: node_id(base_store, target)?,
            kind,
        });
    }
    graph_edges.sort_unstable();
    graph_edges.dedup();
    Ok(graph_edges)
}

fn node_id(
    base_store: &mut RepositoryStoreFile,
    key: NodeKey,
) -> Result<crate::synthetic::NodeId, IncrementalError> {
    base_store
        .node_id(key)?
        .ok_or(RepositoryCompileError::MissingEdgeEndpoint { key }.into())
}

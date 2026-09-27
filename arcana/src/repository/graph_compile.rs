use std::collections::BTreeMap;

use crate::repository_store::CompactRepositoryBuild;
use crate::synthetic::{Edge, EdgeKind, GraphDataset, NodeId};

use super::{NodeFact, NodeKey, RepositoryCompileError, RepositoryFacts, relation_to_edge_kind};

/// Dense graph state compiled without cloning catalogue metadata or unresolved
/// reference payloads. Dense node ID is the index into `node_keys`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledRepositoryGraph {
    pub dataset: GraphDataset,
    pub node_keys: Vec<NodeKey>,
}

impl CompiledRepositoryGraph {
    pub fn node_id(&self, key: NodeKey) -> Option<NodeId> {
        self.node_keys
            .binary_search(&key)
            .ok()
            .and_then(|index| u32::try_from(index).ok())
            .map(NodeId)
    }
}

pub fn compile_repository_graph(
    facts: &RepositoryFacts,
) -> Result<CompiledRepositoryGraph, RepositoryCompileError> {
    let nodes = unique_nodes(&facts.nodes)?;
    let node_ids = dense_node_ids(nodes.keys().copied())?;
    let node_keys = node_ids.keys().copied().collect();
    let edges = compile_edges(&facts.edges, |key| node_ids.get(&key).copied())?;

    for reference in &facts.unresolved {
        if !node_ids.contains_key(&reference.source) {
            return Err(RepositoryCompileError::MissingUnresolvedSource {
                key: reference.source,
            });
        }
    }

    finish(node_keys, edges)
}

#[allow(dead_code)]
pub(crate) fn compile_compact_repository_graph(
    build: &CompactRepositoryBuild,
) -> Result<CompiledRepositoryGraph, RepositoryCompileError> {
    let node_keys = build.nodes.iter().map(|node| node.key).collect::<Vec<_>>();
    let edges = compile_compact_edges(build)?;

    for reference in &build.unresolved {
        if build.node_id(reference.source).is_none() {
            return Err(RepositoryCompileError::MissingUnresolvedSource {
                key: reference.source,
            });
        }
    }

    finish(node_keys, edges)
}

fn dense_node_ids(
    keys: impl Iterator<Item = NodeKey>,
) -> Result<BTreeMap<NodeKey, NodeId>, RepositoryCompileError> {
    keys.enumerate()
        .map(|(index, key)| {
            u32::try_from(index)
                .map(|value| (key, NodeId(value)))
                .map_err(|_| RepositoryCompileError::NodeIdOverflow)
        })
        .collect()
}

fn compile_edges(
    edges: &[super::EdgeFact],
    mut node_id: impl FnMut(NodeKey) -> Option<NodeId>,
) -> Result<Vec<Edge>, RepositoryCompileError> {
    let mut graph_edges = Vec::with_capacity(edges.len());
    for edge in edges {
        graph_edges.push(Edge {
            source: node_id(edge.source)
                .ok_or(RepositoryCompileError::MissingEdgeEndpoint { key: edge.source })?,
            target: node_id(edge.target)
                .ok_or(RepositoryCompileError::MissingEdgeEndpoint { key: edge.target })?,
            kind: relation_to_edge_kind(&edge.relation),
        });
    }
    canonicalize_graph_edges(&mut graph_edges);
    Ok(graph_edges)
}

fn compile_compact_edges(
    build: &CompactRepositoryBuild,
) -> Result<Vec<Edge>, RepositoryCompileError> {
    let mut graph_edges = Vec::with_capacity(build.edges.len());
    for edge in &build.edges {
        graph_edges.push(Edge {
            source: build
                .node_id(edge.source)
                .ok_or(RepositoryCompileError::MissingEdgeEndpoint { key: edge.source })?,
            target: build
                .node_id(edge.target)
                .ok_or(RepositoryCompileError::MissingEdgeEndpoint { key: edge.target })?,
            kind: EdgeKind(edge.relation_code),
        });
    }
    canonicalize_graph_edges(&mut graph_edges);
    Ok(graph_edges)
}

fn canonicalize_graph_edges(edges: &mut Vec<Edge>) {
    edges.sort_unstable();
    edges.dedup();
}

fn finish(
    node_keys: Vec<NodeKey>,
    edges: Vec<Edge>,
) -> Result<CompiledRepositoryGraph, RepositoryCompileError> {
    let node_count =
        u32::try_from(node_keys.len()).map_err(|_| RepositoryCompileError::NodeIdOverflow)?;
    Ok(CompiledRepositoryGraph {
        dataset: GraphDataset { node_count, edges },
        node_keys,
    })
}

fn unique_nodes(
    nodes: &[NodeFact],
) -> Result<BTreeMap<NodeKey, &NodeFact>, RepositoryCompileError> {
    let mut unique = BTreeMap::new();
    for node in nodes {
        if let Some(previous) = unique.get(&node.key)
            && *previous != node
        {
            return Err(RepositoryCompileError::DuplicateConflictingNode { key: node.key });
        }
        unique.entry(node.key).or_insert(node);
    }
    Ok(unique)
}

#[cfg(test)]
#[path = "graph_compile_tests.rs"]
mod tests;

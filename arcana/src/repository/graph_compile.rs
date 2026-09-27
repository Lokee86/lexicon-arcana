use std::collections::BTreeMap;

use crate::synthetic::{Edge, GraphDataset, NodeId};

use super::{NodeFact, NodeKey, RepositoryCompileError, RepositoryFacts, relation_to_edge_kind};

/// Dense graph state compiled without cloning catalogue metadata or unresolved
/// reference payloads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledRepositoryGraph {
    pub dataset: GraphDataset,
    pub node_ids: BTreeMap<NodeKey, NodeId>,
}

pub fn compile_repository_graph(
    facts: &RepositoryFacts,
) -> Result<CompiledRepositoryGraph, RepositoryCompileError> {
    let nodes = unique_nodes(&facts.nodes)?;
    let node_ids = nodes
        .keys()
        .copied()
        .enumerate()
        .map(|(index, key)| {
            u32::try_from(index)
                .map(|value| (key, NodeId(value)))
                .map_err(|_| RepositoryCompileError::NodeIdOverflow)
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;

    let mut edges = Vec::with_capacity(facts.edges.len());
    for edge in &facts.edges {
        let source = *node_ids
            .get(&edge.source)
            .ok_or(RepositoryCompileError::MissingEdgeEndpoint { key: edge.source })?;
        let target = *node_ids
            .get(&edge.target)
            .ok_or(RepositoryCompileError::MissingEdgeEndpoint { key: edge.target })?;
        edges.push(Edge {
            source,
            target,
            kind: relation_to_edge_kind(&edge.relation),
        });
    }
    edges.sort_unstable();
    edges.dedup();

    for reference in &facts.unresolved {
        if !node_ids.contains_key(&reference.source) {
            return Err(RepositoryCompileError::MissingUnresolvedSource {
                key: reference.source,
            });
        }
    }

    let node_count =
        u32::try_from(node_ids.len()).map_err(|_| RepositoryCompileError::NodeIdOverflow)?;
    Ok(CompiledRepositoryGraph {
        dataset: GraphDataset { node_count, edges },
        node_ids,
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

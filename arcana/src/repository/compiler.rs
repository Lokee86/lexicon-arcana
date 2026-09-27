use std::collections::BTreeMap;
use std::fmt;

use crate::synthetic::{Edge, GraphDataset, NodeId};

use super::catalogue::{CatalogueEntry, CatalogueError, RepositoryCatalogue};
use super::relation_codes::relation_to_edge_kind;
use super::{EdgeFact, NodeFact, NodeKey, RelationKind, RepositoryFacts, UnresolvedReferenceFact};

#[cfg(test)]
std::thread_local! {
    static COMPILE_INVOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The compiled graph and its metadata catalogue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledRepository {
    pub dataset: GraphDataset,
    pub node_ids: BTreeMap<NodeKey, NodeId>,
    pub catalogue: RepositoryCatalogue,
    pub unresolved: Vec<UnresolvedReferenceFact>,
}

/// A repository fact set that cannot be compiled into a dense graph.
#[derive(Debug)]
pub enum RepositoryCompileError {
    DuplicateConflictingNode {
        key: NodeKey,
    },
    MissingEdgeEndpoint {
        key: NodeKey,
    },
    MissingUnresolvedSource {
        key: NodeKey,
    },
    DuplicateEdge {
        source: NodeKey,
        target: NodeKey,
        relation: RelationKind,
    },
    NodeIdOverflow,
    Catalogue(CatalogueError),
}

impl fmt::Display for RepositoryCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateConflictingNode { key } => {
                write!(formatter, "node key {key:?} has conflicting facts")
            }
            Self::MissingEdgeEndpoint { key } => {
                write!(formatter, "edge references missing node key {key:?}")
            }
            Self::MissingUnresolvedSource { key } => write!(
                formatter,
                "unresolved reference has missing source node key {key:?}"
            ),
            Self::DuplicateEdge {
                source,
                target,
                relation,
            } => write!(
                formatter,
                "duplicate {relation:?} edge {source:?} -> {target:?}"
            ),
            Self::NodeIdOverflow => {
                formatter.write_str("repository node count exceeds u32 capacity")
            }
            Self::Catalogue(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for RepositoryCompileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Catalogue(error) => Some(error),
            _ => None,
        }
    }
}

/// Compiles borrowed facts into a deterministic dense graph and catalogue.
///
/// This compatibility path preserves the caller's facts, so catalogue node
/// metadata and unresolved references must be cloned into the compiled result.
/// Full rebuilds should prefer the consuming compiler.
pub fn compile_repository_facts(
    facts: &RepositoryFacts,
) -> Result<CompiledRepository, RepositoryCompileError> {
    count_compile();
    let nodes = unique_node_refs(&facts.nodes)?;
    let node_ids = dense_node_ids(nodes.keys().copied())?;
    let graph_edges = compile_edges(&node_ids, &facts.edges)?;
    let mut unresolved = facts.unresolved.clone();
    validate_unresolved(&node_ids, &mut unresolved)?;

    let entries = nodes
        .into_iter()
        .map(|(key, fact)| CatalogueEntry {
            node_id: node_ids[&key],
            fact: fact.clone(),
        })
        .collect();
    finish_compile(node_ids, graph_edges, entries, unresolved)
}

/// Compiles and consumes a complete fact set without cloning node metadata or
/// unresolved-reference strings into the compiled representation.
pub fn compile_repository_facts_owned(
    facts: RepositoryFacts,
) -> Result<CompiledRepository, RepositoryCompileError> {
    count_compile();
    let RepositoryFacts {
        nodes,
        edges,
        mut unresolved,
    } = facts;
    let nodes = unique_nodes_owned(nodes)?;
    let node_ids = dense_node_ids(nodes.keys().copied())?;
    let graph_edges = compile_edges(&node_ids, &edges)?;
    validate_unresolved(&node_ids, &mut unresolved)?;

    let entries = nodes
        .into_iter()
        .map(|(key, fact)| CatalogueEntry {
            node_id: node_ids[&key],
            fact,
        })
        .collect();
    finish_compile(node_ids, graph_edges, entries, unresolved)
}

fn finish_compile(
    node_ids: BTreeMap<NodeKey, NodeId>,
    graph_edges: Vec<Edge>,
    entries: Vec<CatalogueEntry>,
    unresolved: Vec<UnresolvedReferenceFact>,
) -> Result<CompiledRepository, RepositoryCompileError> {
    let node_count =
        u32::try_from(node_ids.len()).map_err(|_| RepositoryCompileError::NodeIdOverflow)?;
    let catalogue = RepositoryCatalogue::new(entries).map_err(RepositoryCompileError::Catalogue)?;
    Ok(CompiledRepository {
        dataset: GraphDataset {
            node_count,
            edges: graph_edges,
        },
        node_ids,
        catalogue,
        unresolved,
    })
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
    node_ids: &BTreeMap<NodeKey, NodeId>,
    edges: &[EdgeFact],
) -> Result<Vec<Edge>, RepositoryCompileError> {
    let mut graph_edges = Vec::with_capacity(edges.len());
    for edge in edges {
        let source = *node_ids
            .get(&edge.source)
            .ok_or(RepositoryCompileError::MissingEdgeEndpoint { key: edge.source })?;
        let target = *node_ids
            .get(&edge.target)
            .ok_or(RepositoryCompileError::MissingEdgeEndpoint { key: edge.target })?;
        graph_edges.push(Edge {
            source,
            target,
            kind: relation_to_edge_kind(&edge.relation),
        });
    }
    graph_edges.sort_unstable();
    graph_edges.dedup();
    Ok(graph_edges)
}

fn validate_unresolved(
    node_ids: &BTreeMap<NodeKey, NodeId>,
    unresolved: &mut Vec<UnresolvedReferenceFact>,
) -> Result<(), RepositoryCompileError> {
    unresolved.sort_unstable();
    unresolved.dedup();
    for reference in unresolved {
        if !node_ids.contains_key(&reference.source) {
            return Err(RepositoryCompileError::MissingUnresolvedSource {
                key: reference.source,
            });
        }
    }
    Ok(())
}

fn unique_node_refs(
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

fn unique_nodes_owned(
    nodes: Vec<NodeFact>,
) -> Result<BTreeMap<NodeKey, NodeFact>, RepositoryCompileError> {
    let mut unique = BTreeMap::new();
    for node in nodes {
        if let Some(previous) = unique.get(&node.key)
            && previous != &node
        {
            return Err(RepositoryCompileError::DuplicateConflictingNode { key: node.key });
        }
        unique.entry(node.key).or_insert(node);
    }
    Ok(unique)
}

fn count_compile() {
    #[cfg(test)]
    COMPILE_INVOCATIONS.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
pub(super) fn reset_compile_invocation_count() {
    COMPILE_INVOCATIONS.with(|count| count.set(0));
}

#[cfg(test)]
pub(super) fn compile_invocation_count() -> usize {
    COMPILE_INVOCATIONS.with(std::cell::Cell::get)
}

/// Short alias for compiling a repository fact set.
pub fn compile_facts(
    facts: &RepositoryFacts,
) -> Result<CompiledRepository, RepositoryCompileError> {
    compile_repository_facts(facts)
}

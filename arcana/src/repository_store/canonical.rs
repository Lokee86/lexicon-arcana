use std::collections::BTreeMap;

use crate::repository::{
    EdgeFact, NodeFact, RepositoryFacts, UnresolvedReferenceFact, collect_node_owners, edge_owner,
    node_owner, unresolved_owner,
};

use super::RepositoryStoreWriteError;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ContributionKind {
    Node = 1,
    Edge = 2,
    Unresolved = 3,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Contribution {
    pub kind: ContributionKind,
    pub record_index: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct CanonicalNode<'a> {
    pub fact: &'a NodeFact,
    pub occurrence_count: u32,
}

pub struct CanonicalFacts<'a> {
    pub nodes: Vec<CanonicalNode<'a>>,
    pub edges: Vec<&'a EdgeFact>,
    pub unresolved: Vec<&'a UnresolvedReferenceFact>,
    pub ownership: BTreeMap<String, Vec<Contribution>>,
}

impl<'a> CanonicalFacts<'a> {
    pub fn prepare(facts: &'a RepositoryFacts) -> Result<Self, RepositoryStoreWriteError> {
        let nodes = canonical_nodes(&facts.nodes)?;
        let mut edges = facts.edges.iter().collect::<Vec<_>>();
        edges.sort_unstable();
        let mut unresolved = facts.unresolved.iter().collect::<Vec<_>>();
        unresolved.sort_unstable();

        validate_references(&nodes, &edges, &unresolved)?;
        let node_owners = collect_node_owners(&facts.nodes)?;
        let mut ownership = BTreeMap::<String, Vec<Contribution>>::new();
        for (index, node) in nodes.iter().enumerate() {
            if let Some(path) = node_owner(node.fact)? {
                add(&mut ownership, path, ContributionKind::Node, index)?;
            }
        }
        for (index, edge) in edges.iter().enumerate() {
            if let Some(path) = edge_owner(edge, &node_owners)? {
                add(&mut ownership, path, ContributionKind::Edge, index)?;
            }
        }
        for (index, reference) in unresolved.iter().enumerate() {
            if let Some(path) = unresolved_owner(reference, &node_owners)? {
                add(&mut ownership, path, ContributionKind::Unresolved, index)?;
            }
        }

        Ok(Self {
            nodes,
            edges,
            unresolved,
            ownership,
        })
    }

    pub fn node_count(&self) -> Result<u32, RepositoryStoreWriteError> {
        u32::try_from(self.nodes.len()).map_err(|_| RepositoryStoreWriteError::TooManyNodes)
    }
}

fn validate_references(
    nodes: &[CanonicalNode<'_>],
    edges: &[&EdgeFact],
    unresolved: &[&UnresolvedReferenceFact],
) -> Result<(), RepositoryStoreWriteError> {
    let keys = nodes
        .iter()
        .map(|node| node.fact.key)
        .collect::<std::collections::BTreeSet<_>>();
    for edge in edges {
        for key in [edge.source, edge.target] {
            if !keys.contains(&key) {
                return Err(RepositoryStoreWriteError::MissingEdgeEndpoint { key });
            }
        }
    }
    for reference in unresolved {
        if !keys.contains(&reference.source) {
            return Err(RepositoryStoreWriteError::MissingUnresolvedSource {
                key: reference.source,
            });
        }
    }
    Ok(())
}

fn canonical_nodes(
    nodes: &[NodeFact],
) -> Result<Vec<CanonicalNode<'_>>, RepositoryStoreWriteError> {
    let mut sorted = nodes.iter().collect::<Vec<_>>();
    sorted.sort_unstable();
    let mut output = Vec::new();
    let mut index = 0;
    while index < sorted.len() {
        let fact = sorted[index];
        let mut end = index + 1;
        while end < sorted.len() && sorted[end].key == fact.key {
            if sorted[end] != fact {
                return Err(RepositoryStoreWriteError::DuplicateConflictingNode { key: fact.key });
            }
            end += 1;
        }
        let count = u32::try_from(end - index)
            .map_err(|_| RepositoryStoreWriteError::TooManyNodeOccurrences { key: fact.key })?;
        output.push(CanonicalNode {
            fact,
            occurrence_count: count,
        });
        index = end;
    }
    Ok(output)
}

fn add(
    ownership: &mut BTreeMap<String, Vec<Contribution>>,
    path: String,
    kind: ContributionKind,
    index: usize,
) -> Result<(), RepositoryStoreWriteError> {
    let record_index =
        u64::try_from(index).map_err(|_| RepositoryStoreWriteError::TooManyContributions)?;
    ownership
        .entry(path)
        .or_default()
        .push(Contribution { kind, record_index });
    Ok(())
}

use crate::{GraphDataset, NodeId};

use super::dataset::canonical_edges;
use super::{DatasetError, Neighbor, QueryError};

/// Simple in-memory adjacency used as the packed reader's correctness oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InMemoryGraph {
    node_count: u32,
    edge_count: u64,
    forward: Vec<Vec<Neighbor>>,
    reverse: Vec<Vec<Neighbor>>,
}

impl InMemoryGraph {
    pub fn new(dataset: &GraphDataset) -> Result<Self, DatasetError> {
        let edges = canonical_edges(dataset)?;
        let mut forward = vec![Vec::new(); dataset.node_count as usize];
        let mut reverse = vec![Vec::new(); dataset.node_count as usize];

        for edge in &edges {
            forward[edge.source.0 as usize].push(Neighbor {
                node: edge.target,
                kind: edge.kind,
            });
            reverse[edge.target.0 as usize].push(Neighbor {
                node: edge.source,
                kind: edge.kind,
            });
        }
        for neighbors in &mut forward {
            neighbors.sort_unstable();
        }
        for neighbors in &mut reverse {
            neighbors.sort_unstable();
        }

        Ok(Self {
            node_count: dataset.node_count,
            edge_count: edges.len() as u64,
            forward,
            reverse,
        })
    }

    pub const fn node_count(&self) -> u32 {
        self.node_count
    }

    pub const fn edge_count(&self) -> u64 {
        self.edge_count
    }

    pub fn forward_neighbors(&self, node: NodeId) -> Result<&[Neighbor], QueryError> {
        self.neighbors(node, &self.forward)
    }

    pub fn reverse_neighbors(&self, node: NodeId) -> Result<&[Neighbor], QueryError> {
        self.neighbors(node, &self.reverse)
    }

    fn neighbors<'a>(
        &self,
        node: NodeId,
        adjacency: &'a [Vec<Neighbor>],
    ) -> Result<&'a [Neighbor], QueryError> {
        adjacency
            .get(node.0 as usize)
            .map(Vec::as_slice)
            .ok_or(QueryError::InvalidNode {
                node,
                node_count: self.node_count,
            })
    }
}

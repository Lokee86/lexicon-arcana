use std::collections::VecDeque;
use std::fmt;

use crate::storage::Neighbor;
use crate::{EdgeKind, NodeId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphPath {
    pub nodes: Vec<NodeId>,
    pub kinds: Vec<EdgeKind>,
}

#[derive(Debug)]
pub enum TraversalError<E> {
    InvalidNode { node: NodeId, node_count: u32 },
    Source(E),
}

impl<E: fmt::Display> fmt::Display for TraversalError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNode { node, node_count } => {
                write!(formatter, "node {} is outside {} nodes", node.0, node_count)
            }
            Self::Source(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for TraversalError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidNode { .. } => None,
            Self::Source(error) => Some(error),
        }
    }
}

pub fn shortest_path<E, F>(
    node_count: u32,
    start: NodeId,
    target: NodeId,
    max_depth: usize,
    mut neighbors: F,
) -> Result<Option<GraphPath>, TraversalError<E>>
where
    F: FnMut(NodeId) -> Result<Vec<Neighbor>, E>,
{
    validate_node(start, node_count)?;
    validate_node(target, node_count)?;
    if start == target {
        return Ok(Some(GraphPath {
            nodes: vec![start],
            kinds: Vec::new(),
        }));
    }

    let mut depth = vec![None; node_count as usize];
    let mut parent = vec![None; node_count as usize];
    depth[start.0 as usize] = Some(0);
    let mut queue = VecDeque::from([start]);

    while let Some(node) = queue.pop_front() {
        let current_depth = depth[node.0 as usize].expect("queued nodes have a distance");
        if current_depth >= max_depth {
            continue;
        }
        for neighbor in neighbors(node).map_err(TraversalError::Source)? {
            validate_node(neighbor.node, node_count)?;
            let index = neighbor.node.0 as usize;
            if depth[index].is_some() {
                continue;
            }
            depth[index] = Some(current_depth + 1);
            parent[index] = Some((node, neighbor.kind));
            if neighbor.node == target {
                return Ok(Some(reconstruct_path(start, target, &parent)));
            }
            queue.push_back(neighbor.node);
        }
    }
    Ok(None)
}

fn validate_node<E>(node: NodeId, node_count: u32) -> Result<(), TraversalError<E>> {
    if node.0 < node_count {
        Ok(())
    } else {
        Err(TraversalError::InvalidNode { node, node_count })
    }
}

fn reconstruct_path(
    start: NodeId,
    target: NodeId,
    parent: &[Option<(NodeId, EdgeKind)>],
) -> GraphPath {
    let mut nodes = vec![target];
    let mut kinds = Vec::new();
    let mut current = target;
    while current != start {
        let (previous, kind) = parent[current.0 as usize].expect("target has a parent chain");
        nodes.push(previous);
        kinds.push(kind);
        current = previous;
    }
    nodes.reverse();
    kinds.reverse();
    GraphPath { nodes, kinds }
}

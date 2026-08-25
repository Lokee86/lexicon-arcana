use std::collections::VecDeque;
use std::fmt;

use crate::storage::Neighbor;
use crate::{EdgeKind, NodeId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphPath {
    pub nodes: Vec<NodeId>,
    pub kinds: Vec<EdgeKind>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathSearchResult {
    pub paths: Vec<GraphPath>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectedComponents {
    pub components: Vec<Vec<NodeId>>,
    pub component_of: Vec<Option<usize>>,
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

pub fn bfs_distances<E, F>(
    node_count: u32,
    starts: &[NodeId],
    max_depth: usize,
    mut neighbors: F,
) -> Result<Vec<Option<usize>>, TraversalError<E>>
where
    F: FnMut(NodeId) -> Result<Vec<Neighbor>, E>,
{
    let mut distances = vec![None; node_count as usize];
    let mut queue = VecDeque::new();
    for &start in starts {
        validate_node(start, node_count)?;
        if distances[start.0 as usize].is_none() {
            distances[start.0 as usize] = Some(0);
            queue.push_back(start);
        }
    }
    while let Some(node) = queue.pop_front() {
        let depth = distances[node.0 as usize].expect("queued nodes have a distance");
        if depth >= max_depth {
            continue;
        }
        for neighbor in neighbors(node).map_err(TraversalError::Source)? {
            validate_node(neighbor.node, node_count)?;
            let index = neighbor.node.0 as usize;
            if distances[index].is_none() {
                distances[index] = Some(depth + 1);
                queue.push_back(neighbor.node);
            }
        }
    }
    Ok(distances)
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

pub fn bounded_simple_paths<E, F>(
    node_count: u32,
    start: NodeId,
    target: NodeId,
    max_depth: usize,
    limit: usize,
    mut neighbors: F,
) -> Result<PathSearchResult, TraversalError<E>>
where
    F: FnMut(NodeId) -> Result<Vec<Neighbor>, E>,
{
    validate_node(start, node_count)?;
    validate_node(target, node_count)?;
    let mut state = PathSearchState {
        target,
        max_depth,
        limit,
        paths: Vec::new(),
        truncated: false,
        nodes: vec![start],
        kinds: Vec::new(),
        visited: vec![false; node_count as usize],
    };
    state.visited[start.0 as usize] = true;
    walk_paths(node_count, start, &mut neighbors, &mut state)?;
    Ok(PathSearchResult {
        paths: state.paths,
        truncated: state.truncated,
    })
}

pub fn connected_components<E, F>(
    node_count: u32,
    selected: &[NodeId],
    mut neighbors: F,
) -> Result<ConnectedComponents, TraversalError<E>>
where
    F: FnMut(NodeId) -> Result<Vec<NodeId>, E>,
{
    let mut selected = selected.to_vec();
    selected.sort_unstable();
    selected.dedup();
    let mut selected_mask = vec![false; node_count as usize];
    for &node in &selected {
        validate_node(node, node_count)?;
        selected_mask[node.0 as usize] = true;
    }
    let mut adjacency = vec![Vec::new(); node_count as usize];
    for &node in &selected {
        for adjacent in neighbors(node).map_err(TraversalError::Source)? {
            validate_node(adjacent, node_count)?;
            if selected_mask[adjacent.0 as usize] {
                adjacency[node.0 as usize].push(adjacent);
                adjacency[adjacent.0 as usize].push(node);
            }
        }
    }
    for values in &mut adjacency {
        values.sort_unstable();
        values.dedup();
    }
    let mut component_of = vec![None; node_count as usize];
    let mut components = Vec::new();
    for start in selected {
        if component_of[start.0 as usize].is_some() {
            continue;
        }
        let index = components.len();
        let mut nodes = Vec::new();
        let mut queue = VecDeque::from([start]);
        component_of[start.0 as usize] = Some(index);
        while let Some(node) = queue.pop_front() {
            nodes.push(node);
            for &adjacent in &adjacency[node.0 as usize] {
                if component_of[adjacent.0 as usize].is_none() {
                    component_of[adjacent.0 as usize] = Some(index);
                    queue.push_back(adjacent);
                }
            }
        }
        nodes.sort_unstable();
        components.push(nodes);
    }
    Ok(ConnectedComponents {
        components,
        component_of,
    })
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

struct PathSearchState {
    target: NodeId,
    max_depth: usize,
    limit: usize,
    paths: Vec<GraphPath>,
    truncated: bool,
    nodes: Vec<NodeId>,
    kinds: Vec<EdgeKind>,
    visited: Vec<bool>,
}

fn walk_paths<E, F>(
    node_count: u32,
    current: NodeId,
    neighbors: &mut F,
    state: &mut PathSearchState,
) -> Result<(), TraversalError<E>>
where
    F: FnMut(NodeId) -> Result<Vec<Neighbor>, E>,
{
    if state.paths.len() >= state.limit {
        state.truncated = true;
        return Ok(());
    }
    if current == state.target {
        state.paths.push(GraphPath {
            nodes: state.nodes.clone(),
            kinds: state.kinds.clone(),
        });
        return Ok(());
    }
    if state.kinds.len() >= state.max_depth {
        return Ok(());
    }
    for neighbor in neighbors(current).map_err(TraversalError::Source)? {
        validate_node(neighbor.node, node_count)?;
        if state.paths.len() >= state.limit {
            state.truncated = true;
            break;
        }
        let index = neighbor.node.0 as usize;
        if state.visited[index] {
            continue;
        }
        state.visited[index] = true;
        state.nodes.push(neighbor.node);
        state.kinds.push(neighbor.kind);
        walk_paths(node_count, neighbor.node, neighbors, state)?;
        state.kinds.pop();
        state.nodes.pop();
        state.visited[index] = false;
    }
    Ok(())
}

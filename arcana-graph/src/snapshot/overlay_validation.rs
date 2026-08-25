use crate::storage::{Neighbor, PackedGraph, StableHasher, canonical_edges};
use crate::{Edge, GraphDataset, NodeId};

use super::{Direction, OverlayChanges, OverlayError};

#[derive(Clone, Debug)]
pub(super) struct OperationIndex {
    forward: Vec<Vec<Neighbor>>,
    reverse: Vec<Vec<Neighbor>>,
}

impl OperationIndex {
    fn new(node_count: u32, edges: &[Edge]) -> Self {
        let mut forward = vec![Vec::new(); node_count as usize];
        let mut reverse = vec![Vec::new(); node_count as usize];
        for edge in edges {
            forward[edge.source.0 as usize].push(Neighbor {
                node: edge.target,
                kind: edge.kind,
            });
            reverse[edge.target.0 as usize].push(Neighbor {
                node: edge.source,
                kind: edge.kind,
            });
        }
        Self { forward, reverse }
    }

    pub fn neighbors(&self, direction: Direction, node: NodeId) -> &[Neighbor] {
        match direction {
            Direction::Forward => &self.forward[node.0 as usize],
            Direction::Reverse => &self.reverse[node.0 as usize],
        }
    }
}

pub(super) struct ValidatedChanges {
    pub added: Vec<Edge>,
    pub removed: Vec<Edge>,
    pub additions: OperationIndex,
    pub removals: OperationIndex,
    pub visible_edge_count: u64,
    pub visible_dataset_checksum: u64,
}

pub(super) fn validate_changes(
    base: &PackedGraph,
    changes: &OverlayChanges,
) -> Result<ValidatedChanges, OverlayError> {
    let added = canonical_edges(&GraphDataset {
        node_count: base.node_count(),
        edges: changes.added.clone(),
    })?;
    let removed = canonical_edges(&GraphDataset {
        node_count: base.node_count(),
        edges: changes.removed.clone(),
    })?;
    reject_conflicts(&added, &removed)?;
    validate_membership(base, &added, &removed)?;

    let additions = OperationIndex::new(base.node_count(), &added);
    let removals = OperationIndex::new(base.node_count(), &removed);
    let visible_edge_count = base
        .edge_count()
        .checked_sub(removed.len() as u64)
        .and_then(|count| count.checked_add(added.len() as u64))
        .ok_or(OverlayError::SizeOverflow)?;
    let visible_dataset_checksum =
        visible_checksum(base, &additions, &removals, visible_edge_count)?;
    Ok(ValidatedChanges {
        added,
        removed,
        additions,
        removals,
        visible_edge_count,
        visible_dataset_checksum,
    })
}

fn reject_conflicts(added: &[Edge], removed: &[Edge]) -> Result<(), OverlayError> {
    let mut add = 0;
    let mut remove = 0;
    while add < added.len() && remove < removed.len() {
        match added[add].cmp(&removed[remove]) {
            std::cmp::Ordering::Less => add += 1,
            std::cmp::Ordering::Greater => remove += 1,
            std::cmp::Ordering::Equal => {
                return Err(OverlayError::OperationConflict { edge: added[add] });
            }
        }
    }
    Ok(())
}

fn validate_membership(
    base: &PackedGraph,
    added: &[Edge],
    removed: &[Edge],
) -> Result<(), OverlayError> {
    for &edge in removed {
        if !base_contains(base, edge) {
            return Err(OverlayError::RemovedEdgeMissing { edge });
        }
    }
    for &edge in added {
        if base_contains(base, edge) {
            return Err(OverlayError::AddedEdgeExists { edge });
        }
    }
    Ok(())
}

fn base_contains(base: &PackedGraph, edge: Edge) -> bool {
    let neighbor = Neighbor {
        node: edge.target,
        kind: edge.kind,
    };
    base.forward_neighbors(edge.source)
        .expect("validated edge source")
        .binary_search(&neighbor)
        .is_ok()
}

fn visible_checksum(
    base: &PackedGraph,
    additions: &OperationIndex,
    removals: &OperationIndex,
    visible_edge_count: u64,
) -> Result<u64, OverlayError> {
    let mut hasher = StableHasher::new();
    hasher.update(&base.node_count().to_le_bytes());
    hasher.update(&visible_edge_count.to_le_bytes());
    for source in 0..base.node_count() {
        let node = NodeId(source);
        let base_neighbors = base.forward_neighbors(node).expect("valid base node");
        let merged = merge_neighbors(
            &base_neighbors,
            removals.neighbors(Direction::Forward, node),
            additions.neighbors(Direction::Forward, node),
        );
        for neighbor in merged {
            hasher.update(&source.to_le_bytes());
            hasher.update(&neighbor.node.0.to_le_bytes());
            hasher.update(&neighbor.kind.0.to_le_bytes());
        }
    }
    Ok(hasher.finish())
}

pub(super) fn merge_neighbors(
    base: &[Neighbor],
    removed: &[Neighbor],
    added: &[Neighbor],
) -> Vec<Neighbor> {
    let mut retained = Vec::with_capacity(base.len().saturating_sub(removed.len()));
    let mut removed_index = 0;
    for &neighbor in base {
        while removed_index < removed.len() && removed[removed_index] < neighbor {
            removed_index += 1;
        }
        if removed.get(removed_index) == Some(&neighbor) {
            removed_index += 1;
        } else {
            retained.push(neighbor);
        }
    }

    let mut merged = Vec::with_capacity(retained.len() + added.len());
    let mut left = retained.into_iter().peekable();
    let mut right = added.iter().copied().peekable();
    while let (Some(base_neighbor), Some(added_neighbor)) = (left.peek(), right.peek()) {
        if base_neighbor < added_neighbor {
            merged.push(left.next().expect("peeked base neighbor"));
        } else {
            merged.push(right.next().expect("peeked added neighbor"));
        }
    }
    merged.extend(left);
    merged.extend(right);
    merged
}

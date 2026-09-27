use crate::snapshot::OverlayChanges;
use crate::synthetic::Edge;

use super::NodeKey;

pub(crate) fn key_difference(
    current: &[NodeKey],
    updated: &[NodeKey],
) -> (Vec<NodeKey>, Vec<NodeKey>) {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut left = 0;
    let mut right = 0;
    while left < current.len() && right < updated.len() {
        match current[left].cmp(&updated[right]) {
            std::cmp::Ordering::Less => {
                removed.push(current[left]);
                left += 1;
            }
            std::cmp::Ordering::Greater => {
                added.push(updated[right]);
                right += 1;
            }
            std::cmp::Ordering::Equal => {
                left += 1;
                right += 1;
            }
        }
    }
    removed.extend_from_slice(&current[left..]);
    added.extend_from_slice(&updated[right..]);
    (added, removed)
}

pub(crate) fn edge_difference(base: &[Edge], visible: &[Edge]) -> OverlayChanges {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut left = 0;
    let mut right = 0;
    while left < base.len() && right < visible.len() {
        match base[left].cmp(&visible[right]) {
            std::cmp::Ordering::Less => {
                removed.push(base[left]);
                left += 1;
            }
            std::cmp::Ordering::Greater => {
                added.push(visible[right]);
                right += 1;
            }
            std::cmp::Ordering::Equal => {
                left += 1;
                right += 1;
            }
        }
    }
    removed.extend_from_slice(&base[left..]);
    added.extend_from_slice(&visible[right..]);
    OverlayChanges { added, removed }
}

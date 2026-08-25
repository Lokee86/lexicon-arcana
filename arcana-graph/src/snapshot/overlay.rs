use std::fs;
use std::path::Path;

use crate::storage::{Direction, Neighbor, PackedGraph, checksum};
use crate::{Edge, EdgeKind, NodeId};

use super::OverlayError;
use super::overlay_format::{
    HEADER_LEN, OverlayHeader, OverlayLayout, get_u16, get_u32, operation_checksum,
};
use super::overlay_validation::{OperationIndex, merge_neighbors, validate_changes};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OverlayChanges {
    pub added: Vec<Edge>,
    pub removed: Vec<Edge>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OverlayWriteSummary {
    pub node_count: u32,
    pub base_edge_count: u64,
    pub added_count: u64,
    pub removed_count: u64,
    pub visible_edge_count: u64,
    pub base_dataset_checksum: u64,
    pub visible_dataset_checksum: u64,
    pub overlay_checksum: u64,
    pub file_len: u64,
}

#[derive(Clone, Debug)]
pub struct GraphOverlay {
    node_count: u32,
    base_edge_count: u64,
    visible_edge_count: u64,
    base_dataset_checksum: u64,
    visible_dataset_checksum: u64,
    overlay_checksum: u64,
    additions: OperationIndex,
    removals: OperationIndex,
}

impl GraphOverlay {
    pub fn open(path: impl AsRef<Path>, base: &PackedGraph) -> Result<Self, OverlayError> {
        let bytes = fs::read(path)?;
        let header = OverlayHeader::decode(&bytes)?;
        validate_file_length(&bytes, header)?;
        validate_layout(header)?;
        let payload = &bytes[usize::from(HEADER_LEN)..];
        let actual_payload_checksum = checksum(payload);
        if actual_payload_checksum != header.payload_checksum {
            return Err(OverlayError::PayloadChecksumMismatch {
                expected: header.payload_checksum,
                actual: actual_payload_checksum,
            });
        }
        validate_base_identity(header, base)?;

        let added = read_edges(&bytes, header.layout.added_edges, header.added_count);
        let removed = read_edges(&bytes, header.layout.removed_edges, header.removed_count);
        validate_canonical(&added, "added")?;
        validate_canonical(&removed, "removed")?;
        let actual_overlay_checksum = operation_checksum(
            header.node_count,
            header.base_edge_count,
            header.base_dataset_checksum,
            &added,
            &removed,
        );
        if actual_overlay_checksum != header.overlay_checksum {
            return Err(OverlayError::OverlayChecksumMismatch {
                expected: header.overlay_checksum,
                actual: actual_overlay_checksum,
            });
        }

        let validated = validate_changes(base, &OverlayChanges { added, removed })?;
        if validated.visible_edge_count != header.visible_edge_count {
            return Err(OverlayError::VisibleEdgeCountMismatch {
                expected: header.visible_edge_count,
                actual: validated.visible_edge_count,
            });
        }
        if validated.visible_dataset_checksum != header.visible_dataset_checksum {
            return Err(OverlayError::VisibleChecksumMismatch {
                expected: header.visible_dataset_checksum,
                actual: validated.visible_dataset_checksum,
            });
        }
        Ok(Self {
            node_count: header.node_count,
            base_edge_count: header.base_edge_count,
            visible_edge_count: header.visible_edge_count,
            base_dataset_checksum: header.base_dataset_checksum,
            visible_dataset_checksum: header.visible_dataset_checksum,
            overlay_checksum: header.overlay_checksum,
            additions: validated.additions,
            removals: validated.removals,
        })
    }

    pub const fn node_count(&self) -> u32 {
        self.node_count
    }

    pub const fn base_edge_count(&self) -> u64 {
        self.base_edge_count
    }

    pub const fn visible_edge_count(&self) -> u64 {
        self.visible_edge_count
    }

    pub const fn base_dataset_checksum(&self) -> u64 {
        self.base_dataset_checksum
    }

    pub const fn visible_dataset_checksum(&self) -> u64 {
        self.visible_dataset_checksum
    }

    pub const fn overlay_checksum(&self) -> u64 {
        self.overlay_checksum
    }

    pub(crate) fn merge_owned(
        &self,
        direction: Direction,
        node: NodeId,
        base_neighbors: Vec<Neighbor>,
    ) -> Vec<Neighbor> {
        let removed = self.removals.neighbors(direction, node);
        let added = self.additions.neighbors(direction, node);
        if removed.is_empty() && added.is_empty() {
            return base_neighbors;
        }
        merge_neighbors(&base_neighbors, removed, added)
    }
}

fn validate_file_length(bytes: &[u8], header: OverlayHeader) -> Result<(), OverlayError> {
    let actual = bytes.len() as u64;
    if header.layout.file_len != actual {
        return Err(OverlayError::FileLengthMismatch {
            declared: header.layout.file_len,
            actual,
        });
    }
    Ok(())
}

fn validate_layout(header: OverlayHeader) -> Result<(), OverlayError> {
    let expected = OverlayLayout::for_counts(header.added_count, header.removed_count)?;
    for (section, actual, expected) in [
        ("added", header.layout.added_edges, expected.added_edges),
        (
            "removed",
            header.layout.removed_edges,
            expected.removed_edges,
        ),
        ("file length", header.layout.file_len, expected.file_len),
    ] {
        if actual != expected {
            return Err(OverlayError::LayoutMismatch { section });
        }
    }
    Ok(())
}

fn validate_base_identity(header: OverlayHeader, base: &PackedGraph) -> Result<(), OverlayError> {
    if header.node_count != base.node_count() {
        return Err(OverlayError::BaseNodeCountMismatch {
            expected: header.node_count,
            actual: base.node_count(),
        });
    }
    if header.base_edge_count != base.edge_count() {
        return Err(OverlayError::BaseEdgeCountMismatch {
            expected: header.base_edge_count,
            actual: base.edge_count(),
        });
    }
    if header.base_dataset_checksum != base.dataset_checksum() {
        return Err(OverlayError::BaseChecksumMismatch {
            expected: header.base_dataset_checksum,
            actual: base.dataset_checksum(),
        });
    }
    Ok(())
}

fn read_edges(bytes: &[u8], start: u64, count: u64) -> Vec<Edge> {
    let mut edges = Vec::with_capacity(count as usize);
    for index in 0..count {
        let offset = (start + index * 10) as usize;
        edges.push(Edge {
            source: NodeId(get_u32(bytes, offset)),
            target: NodeId(get_u32(bytes, offset + 4)),
            kind: EdgeKind(get_u16(bytes, offset + 8)),
        });
    }
    edges
}

fn validate_canonical(edges: &[Edge], section: &'static str) -> Result<(), OverlayError> {
    if edges.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(OverlayError::UnsortedOperations { section });
    }
    Ok(())
}

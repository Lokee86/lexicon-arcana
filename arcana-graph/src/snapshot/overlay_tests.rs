use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::storage::{Direction, PackedGraph, write_packed};
use crate::{Edge, EdgeKind, GraphDataset, NodeId};

use super::{GraphOverlay, OverlayChanges, OverlayError, write_overlay};

static PATH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempPath(PathBuf);

impl TempPath {
    fn new(extension: &str) -> Self {
        let sequence = PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "arcana-overlay-test-{}-{sequence}.{extension}",
            std::process::id()
        )))
    }

    fn as_path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn edge(source: u32, target: u32, kind: u16) -> Edge {
    Edge {
        source: NodeId(source),
        target: NodeId(target),
        kind: EdgeKind(kind),
    }
}

fn base_dataset() -> GraphDataset {
    GraphDataset {
        node_count: 6,
        edges: vec![
            edge(0, 1, 1),
            edge(0, 2, 1),
            edge(1, 3, 2),
            edge(2, 3, 1),
            edge(3, 4, 3),
        ],
    }
}

fn open_base(path: &TempPath) -> PackedGraph {
    write_packed(path.as_path(), &base_dataset()).unwrap();
    PackedGraph::open(path.as_path()).unwrap()
}

fn changes() -> OverlayChanges {
    OverlayChanges {
        removed: vec![edge(0, 2, 1), edge(3, 4, 3)],
        added: vec![edge(0, 5, 4), edge(4, 1, 2)],
    }
}

#[test]
fn overlay_round_trips_and_merges_both_directions() {
    let base_path = TempPath::new("pack");
    let overlay_path = TempPath::new("overlay");
    let base = open_base(&base_path);
    let summary = write_overlay(overlay_path.as_path(), &base, &changes()).unwrap();
    let overlay = GraphOverlay::open(overlay_path.as_path(), &base).unwrap();

    assert_eq!(summary.visible_edge_count, 5);
    assert_eq!(summary.overlay_checksum, overlay.overlay_checksum());
    assert_eq!(
        summary.visible_dataset_checksum,
        overlay.visible_dataset_checksum()
    );
    assert_eq!(
        overlay.merge_owned(
            Direction::Forward,
            NodeId(0),
            base.forward_neighbors(NodeId(0)).unwrap()
        ),
        vec![
            super::Neighbor {
                node: NodeId(1),
                kind: EdgeKind(1)
            },
            super::Neighbor {
                node: NodeId(5),
                kind: EdgeKind(4)
            },
        ]
    );
    assert_eq!(
        overlay.merge_owned(
            Direction::Reverse,
            NodeId(1),
            base.reverse_neighbors(NodeId(1)).unwrap()
        ),
        vec![
            super::Neighbor {
                node: NodeId(0),
                kind: EdgeKind(1)
            },
            super::Neighbor {
                node: NodeId(4),
                kind: EdgeKind(2)
            },
        ]
    );
}

#[test]
fn operation_order_does_not_change_overlay_bytes() {
    let base_path = TempPath::new("pack");
    let first_path = TempPath::new("overlay");
    let second_path = TempPath::new("overlay");
    let base = open_base(&base_path);
    let first = changes();
    let mut second = changes();
    second.added.reverse();
    second.removed.reverse();

    write_overlay(first_path.as_path(), &base, &first).unwrap();
    write_overlay(second_path.as_path(), &base, &second).unwrap();
    assert_eq!(
        fs::read(first_path.as_path()).unwrap(),
        fs::read(second_path.as_path()).unwrap()
    );
}

#[test]
fn overlay_rejects_invalid_membership_and_conflicts() {
    let base_path = TempPath::new("pack");
    let base = open_base(&base_path);
    let cases = [
        (
            OverlayChanges {
                added: Vec::new(),
                removed: vec![edge(5, 0, 1)],
            },
            "missing",
        ),
        (
            OverlayChanges {
                added: vec![edge(0, 1, 1)],
                removed: Vec::new(),
            },
            "existing",
        ),
        (
            OverlayChanges {
                added: vec![edge(0, 1, 1)],
                removed: vec![edge(0, 1, 1)],
            },
            "conflict",
        ),
    ];

    for (changes, label) in cases {
        let path = TempPath::new(label);
        let error = write_overlay(path.as_path(), &base, &changes).unwrap_err();
        assert!(matches!(
            error,
            OverlayError::RemovedEdgeMissing { .. }
                | OverlayError::AddedEdgeExists { .. }
                | OverlayError::OperationConflict { .. }
        ));
        assert!(!path.as_path().exists());
    }
}

#[test]
fn overlay_is_bound_to_one_base_identity() {
    let base_path = TempPath::new("pack");
    let other_path = TempPath::new("pack");
    let overlay_path = TempPath::new("overlay");
    let base = open_base(&base_path);
    write_overlay(overlay_path.as_path(), &base, &changes()).unwrap();

    let mut other_dataset = base_dataset();
    other_dataset.edges.push(edge(5, 0, 9));
    write_packed(other_path.as_path(), &other_dataset).unwrap();
    let other = PackedGraph::open(other_path.as_path()).unwrap();
    assert!(matches!(
        GraphOverlay::open(overlay_path.as_path(), &other),
        Err(OverlayError::BaseEdgeCountMismatch { .. } | OverlayError::BaseChecksumMismatch { .. })
    ));
}

#[test]
fn overlay_detects_payload_corruption_and_refuses_overwrite() {
    let base_path = TempPath::new("pack");
    let overlay_path = TempPath::new("overlay");
    let base = open_base(&base_path);
    write_overlay(overlay_path.as_path(), &base, &changes()).unwrap();
    assert!(matches!(
        write_overlay(overlay_path.as_path(), &base, &changes()),
        Err(OverlayError::Io(error)) if error.kind() == io::ErrorKind::AlreadyExists
    ));

    let mut bytes = fs::read(overlay_path.as_path()).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x55;
    fs::write(overlay_path.as_path(), bytes).unwrap();
    assert!(matches!(
        GraphOverlay::open(overlay_path.as_path(), &base),
        Err(OverlayError::PayloadChecksumMismatch { .. })
    ));
}

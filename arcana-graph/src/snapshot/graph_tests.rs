use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::storage::{InMemoryGraph, write_packed};
use crate::{Edge, EdgeKind, GraphDataset, NodeId};

use super::{GraphSnapshot, OverlayChanges, SnapshotError, publish_snapshot, write_overlay};

static DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let sequence = DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "arcana-snapshot-test-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.0.join(path)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
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

#[test]
fn base_only_snapshot_publishes_and_queries() {
    let directory = TempDirectory::new();
    let base_path = directory.join("base.pack");
    let manifest_path = directory.join("snapshot.manifest");
    let dataset = base_dataset();
    let oracle = InMemoryGraph::new(&dataset).unwrap();
    write_packed(&base_path, &dataset).unwrap();

    let manifest = publish_snapshot(&manifest_path, "base.pack", None, 123).unwrap();
    let snapshot = GraphSnapshot::open(&manifest_path).unwrap();
    assert_eq!(manifest.snapshot_id, snapshot.snapshot_id());
    assert_eq!(snapshot.edge_count(), dataset.edges.len() as u64);
    for node in 0..dataset.node_count {
        let node = NodeId(node);
        assert_eq!(
            snapshot.forward_neighbors(node).unwrap(),
            oracle.forward_neighbors(node).unwrap()
        );
        assert_eq!(
            snapshot.reverse_neighbors(node).unwrap(),
            oracle.reverse_neighbors(node).unwrap()
        );
    }
}

#[test]
fn visible_neighbor_iterators_match_vec_apis() {
    let directory = TempDirectory::new();
    let base_path = directory.join("base.pack");
    let base_manifest_path = directory.join("base.manifest");
    let overlay_path = directory.join("change.overlay");
    let overlay_manifest_path = directory.join("overlay.manifest");
    let dataset = base_dataset();
    write_packed(&base_path, &dataset).unwrap();
    publish_snapshot(&base_manifest_path, "base.pack", None, 123).unwrap();

    let base_snapshot = GraphSnapshot::open(&base_manifest_path).unwrap();
    for node in 0..dataset.node_count {
        let node = NodeId(node);
        assert_eq!(
            base_snapshot
                .forward_neighbors_iter(node)
                .unwrap()
                .collect::<Vec<_>>(),
            base_snapshot.forward_neighbors(node).unwrap()
        );
        assert_eq!(
            base_snapshot
                .reverse_neighbors_iter(node)
                .unwrap()
                .collect::<Vec<_>>(),
            base_snapshot.reverse_neighbors(node).unwrap()
        );
    }

    let base = crate::storage::PackedGraph::open(&base_path).unwrap();
    let changes = OverlayChanges {
        removed: vec![edge(0, 2, 1)],
        added: vec![edge(0, 5, 4)],
    };
    write_overlay(&overlay_path, &base, &changes).unwrap();
    publish_snapshot(
        &overlay_manifest_path,
        "base.pack",
        Some(Path::new("change.overlay")),
        456,
    )
    .unwrap();

    let overlay_snapshot = GraphSnapshot::open(&overlay_manifest_path).unwrap();
    for node in 0..dataset.node_count {
        let node = NodeId(node);
        assert_eq!(
            overlay_snapshot
                .forward_neighbors_iter(node)
                .unwrap()
                .collect::<Vec<_>>(),
            overlay_snapshot.forward_neighbors(node).unwrap()
        );
        assert_eq!(
            overlay_snapshot
                .reverse_neighbors_iter(node)
                .unwrap()
                .collect::<Vec<_>>(),
            overlay_snapshot.reverse_neighbors(node).unwrap()
        );
    }
}

#[test]
fn overlay_snapshot_matches_materialized_graph() {
    let directory = TempDirectory::new();
    let base_path = directory.join("base.pack");
    let overlay_path = directory.join("change.overlay");
    let manifest_path = directory.join("snapshot.manifest");
    let base_dataset = base_dataset();
    write_packed(&base_path, &base_dataset).unwrap();
    let base = crate::storage::PackedGraph::open(&base_path).unwrap();
    let changes = OverlayChanges {
        removed: vec![edge(0, 2, 1), edge(3, 4, 3)],
        added: vec![edge(0, 5, 4), edge(4, 1, 2)],
    };
    write_overlay(&overlay_path, &base, &changes).unwrap();
    let mut visible_edges: Vec<_> = base_dataset
        .edges
        .iter()
        .copied()
        .filter(|edge| !changes.removed.contains(edge))
        .collect();
    visible_edges.extend(changes.added.iter().copied());
    visible_edges.sort_unstable();
    let visible = GraphDataset {
        node_count: base_dataset.node_count,
        edges: visible_edges,
    };
    let oracle = InMemoryGraph::new(&visible).unwrap();

    publish_snapshot(
        &manifest_path,
        "base.pack",
        Some(Path::new("change.overlay")),
        456,
    )
    .unwrap();
    let snapshot = GraphSnapshot::open(&manifest_path).unwrap();
    assert_eq!(snapshot.edge_count(), visible.edges.len() as u64);
    for node in 0..visible.node_count {
        let node = NodeId(node);
        assert_eq!(
            snapshot.forward_neighbors(node).unwrap(),
            oracle.forward_neighbors(node).unwrap()
        );
        assert_eq!(
            snapshot.reverse_neighbors(node).unwrap(),
            oracle.reverse_neighbors(node).unwrap()
        );
    }
}

#[test]
fn opening_rejects_manifest_identity_tampering() {
    let directory = TempDirectory::new();
    let base_path = directory.join("base.pack");
    let manifest_path = directory.join("snapshot.manifest");
    write_packed(&base_path, &base_dataset()).unwrap();
    publish_snapshot(&manifest_path, "base.pack", None, 123).unwrap();
    let text = fs::read_to_string(&manifest_path).unwrap();
    fs::write(
        &manifest_path,
        text.replace("base_edge_count=5", "base_edge_count=4"),
    )
    .unwrap();
    assert!(matches!(
        GraphSnapshot::open(&manifest_path),
        Err(SnapshotError::ManifestMismatch {
            field: "base_edge_count",
            ..
        })
    ));
}

#[test]
fn publication_rejects_parent_traversal() {
    let directory = TempDirectory::new();
    let manifest_path = directory.join("snapshot.manifest");
    assert!(matches!(
        publish_snapshot(&manifest_path, "../base.pack", None, 0),
        Err(SnapshotError::InvalidRelativePath {
            field: "base_file",
            ..
        })
    ));
}

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::storage::{PackedGraph, write_packed};
use crate::{Edge, EdgeKind, GraphDataset, NodeId};

use super::{
    GraphSnapshot, OverlayChanges, SnapshotError, compact_snapshot, publish_snapshot, write_overlay,
};

static DIRECTORY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let sequence = DIRECTORY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "arcana-compaction-test-{}-{sequence}",
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

fn dataset() -> GraphDataset {
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

fn create_overlay_snapshot(directory: &TempDirectory) -> PathBuf {
    let base_path = directory.join("base.pack");
    let overlay_path = directory.join("change.overlay");
    let manifest_path = directory.join("source.manifest");
    write_packed(&base_path, &dataset()).unwrap();
    let base = PackedGraph::open(&base_path).unwrap();
    write_overlay(
        &overlay_path,
        &base,
        &OverlayChanges {
            removed: vec![edge(0, 2, 1), edge(3, 4, 3)],
            added: vec![edge(0, 5, 4), edge(4, 1, 2)],
        },
    )
    .unwrap();
    publish_snapshot(
        &manifest_path,
        "base.pack",
        Some(Path::new("change.overlay")),
        1,
    )
    .unwrap();
    manifest_path
}

#[test]
fn compaction_preserves_identity_and_all_queries() {
    let directory = TempDirectory::new();
    let source_path = create_overlay_snapshot(&directory);
    let source = GraphSnapshot::open(&source_path).unwrap();
    let output_path = directory.join("compacted.manifest");

    let manifest = compact_snapshot(&source_path, &output_path, "compacted.pack", 2).unwrap();
    let compacted = GraphSnapshot::open(&output_path).unwrap();
    assert!(manifest.overlay_file.is_none());
    assert_eq!(source.snapshot_id(), compacted.snapshot_id());
    assert_eq!(source.edge_count(), compacted.edge_count());
    assert_eq!(source.dataset_checksum(), compacted.dataset_checksum());
    for node in 0..source.node_count() {
        let node = NodeId(node);
        assert_eq!(
            source.forward_neighbors(node).unwrap(),
            compacted.forward_neighbors(node).unwrap()
        );
        assert_eq!(
            source.reverse_neighbors(node).unwrap(),
            compacted.reverse_neighbors(node).unwrap()
        );
    }
    assert!(source_path.exists());
    assert!(directory.join("base.pack").exists());
    assert!(directory.join("change.overlay").exists());
}

#[test]
fn existing_output_manifest_prevents_base_creation() {
    let directory = TempDirectory::new();
    let source_path = create_overlay_snapshot(&directory);
    let output_path = directory.join("compacted.manifest");
    fs::write(&output_path, "occupied").unwrap();

    assert!(matches!(
        compact_snapshot(&source_path, &output_path, "compacted.pack", 2),
        Err(SnapshotError::Io(error)) if error.kind() == io::ErrorKind::AlreadyExists
    ));
    assert!(!directory.join("compacted.pack").exists());
}

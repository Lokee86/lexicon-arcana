use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::storage::{DatasetError, InMemoryGraph, PackedError, PackedGraph, write_packed};
use crate::{Edge, EdgeKind, GraphDataset, NodeId};

static PATH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempPath(PathBuf);

impl TempPath {
    fn new(label: &str) -> Self {
        let sequence = PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "arcana-graph-{label}-{}-{sequence}.pack",
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

fn representative_dataset() -> GraphDataset {
    GraphDataset {
        node_count: 10,
        edges: vec![
            edge(0, 1, 1),
            edge(0, 2, 2),
            edge(0, 2, 3),
            edge(1, 3, 1),
            edge(2, 3, 2),
            edge(3, 4, 4),
            edge(4, 1, 5),
            edge(4, 8, 6),
            edge(8, 9, 7),
            edge(9, 5, 8),
        ],
    }
}

fn assert_round_trip(dataset: &GraphDataset, label: &str) {
    let path = TempPath::new(label);
    let oracle = InMemoryGraph::new(dataset).expect("valid reference graph");
    let summary = write_packed(path.as_path(), dataset).expect("packed write succeeds");
    let packed = PackedGraph::open(path.as_path()).expect("packed graph opens");

    assert_eq!(packed.node_count(), oracle.node_count());
    assert_eq!(packed.edge_count(), oracle.edge_count());
    assert_eq!(summary.dataset_checksum, packed.dataset_checksum());
    assert_eq!(
        summary.file_len,
        fs::metadata(path.as_path()).unwrap().len()
    );

    for node in 0..dataset.node_count {
        let node = NodeId(node);
        assert_eq!(
            packed.forward_neighbors(node).unwrap(),
            oracle.forward_neighbors(node).unwrap()
        );
        assert_eq!(
            packed.reverse_neighbors(node).unwrap(),
            oracle.reverse_neighbors(node).unwrap()
        );
    }
}

#[test]
fn representative_graph_round_trips() {
    assert_round_trip(&representative_dataset(), "representative");
}

#[test]
fn empty_adjacency_and_parallel_kinds_round_trip() {
    assert_round_trip(
        &GraphDataset {
            node_count: 5,
            edges: Vec::new(),
        },
        "empty",
    );
    assert_round_trip(
        &GraphDataset {
            node_count: 4,
            edges: vec![edge(0, 1, 2), edge(2, 0, 3), edge(0, 1, 1)],
        },
        "parallel-kinds",
    );
}

#[test]
fn borrowed_neighbor_iterator_matches_owned_api() {
    let dataset = representative_dataset();
    let path = TempPath::new("borrowed-neighbors");
    write_packed(path.as_path(), &dataset).unwrap();
    let packed = PackedGraph::open(path.as_path()).unwrap();

    let forward: Vec<_> = packed.forward_neighbors_iter(NodeId(0)).unwrap().collect();
    let reverse: Vec<_> = packed.reverse_neighbors_iter(NodeId(1)).unwrap().collect();
    assert_eq!(forward, packed.forward_neighbors(NodeId(0)).unwrap());
    assert_eq!(reverse, packed.reverse_neighbors(NodeId(1)).unwrap());
}

#[test]
fn logical_edge_order_does_not_change_packed_bytes() {
    let dataset = representative_dataset();
    let mut reordered = dataset.clone();
    reordered.edges.reverse();
    let first = TempPath::new("deterministic-a");
    let second = TempPath::new("deterministic-b");

    write_packed(first.as_path(), &dataset).unwrap();
    write_packed(second.as_path(), &reordered).unwrap();
    assert_eq!(
        fs::read(first.as_path()).unwrap(),
        fs::read(second.as_path()).unwrap()
    );
}

#[test]
fn self_edges_round_trip() {
    assert_round_trip(
        &GraphDataset {
            node_count: 2,
            edges: vec![edge(1, 1, 7)],
        },
        "self-edge",
    );
}

#[test]
fn writer_rejects_invalid_datasets() {
    let invalid = [
        (
            GraphDataset {
                node_count: 2,
                edges: vec![edge(0, 2, 0)],
            },
            "range",
        ),
        (
            GraphDataset {
                node_count: 2,
                edges: vec![edge(0, 1, 0), edge(0, 1, 0)],
            },
            "duplicate",
        ),
    ];

    for (dataset, label) in invalid {
        let path = TempPath::new(label);
        assert!(matches!(
            write_packed(path.as_path(), &dataset),
            Err(PackedError::Dataset(
                DatasetError::EndpointOutOfRange { .. } | DatasetError::DuplicateEdge { .. }
            ))
        ));
        assert!(!path.as_path().exists());
    }
}

#[test]
fn writer_refuses_to_replace_an_existing_snapshot() {
    let path = TempPath::new("existing");
    fs::write(path.as_path(), b"owned").unwrap();
    let dataset = GraphDataset {
        node_count: 1,
        edges: Vec::new(),
    };

    assert!(matches!(
        write_packed(path.as_path(), &dataset),
        Err(PackedError::Io(error)) if error.kind() == io::ErrorKind::AlreadyExists
    ));
    assert_eq!(fs::read(path.as_path()).unwrap(), b"owned");
}

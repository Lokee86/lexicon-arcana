use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::storage::{InMemoryGraph, Neighbor, PackedGraph, write_packed};
use crate::synthetic::{GraphSpec, NodeId, ScaleTier, Topology, generate};

static PATH_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TempPath(PathBuf);

impl TempPath {
    fn new(label: &str) -> Self {
        let sequence = PATH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "arcana-graph-kernel-{label}-{}-{sequence}.pack",
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

fn topology_specs() -> Vec<GraphSpec> {
    vec![
        GraphSpec {
            topology: Topology::Modular {
                cluster_count: 8,
                cross_cluster_ratio: 2_500,
            },
            node_count: 64,
            edge_count: 300,
            seed: 41,
        },
        GraphSpec {
            topology: Topology::Entangled {
                cluster_count: 8,
                hub_count: 4,
            },
            node_count: 64,
            edge_count: 300,
            seed: 42,
        },
        GraphSpec {
            topology: Topology::HubHeavy { hub_count: 4 },
            node_count: 64,
            edge_count: 300,
            seed: 43,
        },
        GraphSpec {
            topology: Topology::Layered { layer_count: 8 },
            node_count: 64,
            edge_count: 300,
            seed: 44,
        },
        GraphSpec {
            topology: Topology::DenseSubsystem {
                dense_node_count: 16,
            },
            node_count: 64,
            edge_count: 300,
            seed: 45,
        },
    ]
}

#[test]
fn every_synthetic_topology_round_trips_through_shared_storage() {
    for (index, spec) in topology_specs().iter().enumerate() {
        let dataset = generate(spec).expect("valid synthetic graph");
        let oracle = InMemoryGraph::new(&dataset).expect("valid reference graph");
        let path = TempPath::new(&format!("topology-{index}"));
        write_packed(path.as_path(), &dataset).expect("packed write succeeds");
        let packed = PackedGraph::open(path.as_path()).expect("packed graph opens");

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
}

#[test]
#[ignore = "medium-scale storage smoke"]
fn medium_scale_shared_storage_smoke() {
    let dataset = generate(&GraphSpec::for_tier(
        Topology::Modular {
            cluster_count: 1_000,
            cross_cluster_ratio: 2_500,
        },
        ScaleTier::Medium,
        77,
    ))
    .expect("valid medium graph");
    let path = TempPath::new("medium-scale");
    let summary = write_packed(path.as_path(), &dataset).expect("medium packed write");
    let packed = PackedGraph::open(path.as_path()).expect("medium packed open");

    assert_eq!(summary.node_count, 100_000);
    assert_eq!(summary.edge_count, 1_000_000);
    for node in [0, 1, 999, 50_000, 99_999] {
        let mut forward: Vec<Neighbor> = dataset
            .edges
            .iter()
            .filter(|edge| edge.source == NodeId(node))
            .map(|edge| Neighbor {
                node: edge.target,
                kind: edge.kind,
            })
            .collect();
        forward.sort_unstable();
        assert_eq!(packed.forward_neighbors(NodeId(node)).unwrap(), forward);
    }
}

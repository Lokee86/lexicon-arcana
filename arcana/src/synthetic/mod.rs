//! Deterministic synthetic graph generation primitives.

mod dense;
mod entangled;
mod generator;
mod hub_heavy;
mod layered;
mod modular;
mod mutation;
mod sampling;
mod spec;
#[cfg(test)]
mod spec_tests;

pub use arcana_graph::{Edge, EdgeKind, GraphDataset, NodeId};
pub use generator::generate;
pub use mutation::{GraphMutation, MutationError, MutationScenario, apply_mutation, plan_mutation};
pub use spec::{GraphSpec, GraphSpecError, ScaleTier, Topology};

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{Edge, GraphSpec, Topology, generate};

    fn specs(seed: u64) -> Vec<GraphSpec> {
        vec![
            GraphSpec {
                topology: Topology::Modular {
                    cluster_count: 8,
                    cross_cluster_ratio: 2_500,
                },
                node_count: 64,
                edge_count: 300,
                seed,
            },
            GraphSpec {
                topology: Topology::Entangled {
                    cluster_count: 8,
                    hub_count: 4,
                },
                node_count: 64,
                edge_count: 300,
                seed,
            },
            GraphSpec {
                topology: Topology::HubHeavy { hub_count: 4 },
                node_count: 64,
                edge_count: 300,
                seed,
            },
            GraphSpec {
                topology: Topology::Layered { layer_count: 8 },
                node_count: 64,
                edge_count: 300,
                seed,
            },
            GraphSpec {
                topology: Topology::DenseSubsystem {
                    dense_node_count: 16,
                },
                node_count: 64,
                edge_count: 300,
                seed,
            },
        ]
    }

    #[test]
    fn every_topology_is_deterministic_and_canonical() {
        for spec in specs(7) {
            let first = generate(&spec).expect("valid topology specification");
            let second = generate(&spec).expect("valid topology specification");
            let unique: HashSet<Edge> = first.edges.iter().copied().collect();

            assert_eq!(first, second);
            assert_eq!(first.edges.len(), spec.edge_count as usize);
            assert_eq!(unique.len(), first.edges.len());
            assert!(first.edges.iter().all(|edge| edge.source != edge.target));
            assert!(first.edges.windows(2).all(|pair| pair[0] < pair[1]));
        }
    }

    #[test]
    fn topology_families_produce_distinct_datasets() {
        let datasets: Vec<_> = specs(9)
            .iter()
            .map(|spec| generate(spec).expect("valid topology specification"))
            .collect();

        for left in 0..datasets.len() {
            for right in left + 1..datasets.len() {
                assert_ne!(datasets[left], datasets[right]);
            }
        }
    }

    #[test]
    fn different_seeds_produce_different_datasets() {
        for (first, second) in specs(7).iter().zip(specs(8)) {
            assert_ne!(generate(first), generate(&second));
        }
    }

    #[test]
    fn large_sparse_topologies_do_not_enumerate_pair_capacity() {
        let specs = [
            GraphSpec {
                topology: Topology::Modular {
                    cluster_count: 1_000,
                    cross_cluster_ratio: 2_500,
                },
                node_count: 1_000_000,
                edge_count: 10_000,
                seed: 31,
            },
            GraphSpec {
                topology: Topology::Entangled {
                    cluster_count: 1_000,
                    hub_count: 16,
                },
                node_count: 1_000_000,
                edge_count: 10_000,
                seed: 31,
            },
            GraphSpec {
                topology: Topology::HubHeavy { hub_count: 16 },
                node_count: 1_000_000,
                edge_count: 10_000,
                seed: 31,
            },
            GraphSpec {
                topology: Topology::Layered { layer_count: 64 },
                node_count: 1_000_000,
                edge_count: 10_000,
                seed: 31,
            },
            GraphSpec {
                topology: Topology::DenseSubsystem {
                    dense_node_count: 1_000,
                },
                node_count: 1_000_000,
                edge_count: 10_000,
                seed: 31,
            },
        ];

        for spec in specs {
            assert_eq!(
                generate(&spec)
                    .expect("large sparse topology should be valid")
                    .edges
                    .len(),
                10_000
            );
        }
    }
}

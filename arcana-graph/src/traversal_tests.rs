use std::convert::Infallible;

use crate::storage::{InMemoryGraph, Neighbor};
use crate::traversal::{bfs_distances, bounded_simple_paths, connected_components, shortest_path};
use crate::{Edge, EdgeKind, GraphDataset, NodeId};

fn graph() -> InMemoryGraph {
    InMemoryGraph::new(&GraphDataset {
        node_count: 6,
        edges: vec![
            edge(0, 1, 1),
            edge(0, 2, 2),
            edge(1, 3, 1),
            edge(2, 3, 2),
            edge(3, 4, 3),
            edge(4, 1, 4),
        ],
    })
    .unwrap()
}

fn edge(source: u32, target: u32, kind: u16) -> Edge {
    Edge {
        source: NodeId(source),
        target: NodeId(target),
        kind: EdgeKind(kind),
    }
}

fn outgoing(
    graph: &InMemoryGraph,
    node: NodeId,
) -> Result<Vec<Neighbor>, crate::storage::QueryError> {
    graph.forward_neighbors(node).map(<[Neighbor]>::to_vec)
}

#[test]
fn bfs_and_shortest_path_preserve_directed_edge_kinds() {
    let graph = graph();
    let distances = bfs_distances(graph.node_count(), &[NodeId(0)], 8, |node| {
        outgoing(&graph, node)
    })
    .unwrap();
    assert_eq!(distances[4], Some(3));
    assert_eq!(distances[5], None);

    let path = shortest_path(graph.node_count(), NodeId(0), NodeId(4), 8, |node| {
        outgoing(&graph, node)
    })
    .unwrap()
    .unwrap();
    assert_eq!(path.nodes, vec![NodeId(0), NodeId(1), NodeId(3), NodeId(4)]);
    assert_eq!(path.kinds, vec![EdgeKind(1), EdgeKind(1), EdgeKind(3)]);
}

#[test]
fn bounded_paths_avoid_cycles_and_report_truncation() {
    let graph = graph();
    let result = bounded_simple_paths(graph.node_count(), NodeId(0), NodeId(3), 8, 1, |node| {
        outgoing(&graph, node)
    })
    .unwrap();
    assert_eq!(result.paths.len(), 1);
    assert!(result.truncated);
    assert_eq!(result.paths[0].nodes, vec![NodeId(0), NodeId(1), NodeId(3)]);
}

#[test]
fn connected_components_are_deterministic_and_undirected() {
    let graph = graph();
    let selected = [NodeId(0), NodeId(1), NodeId(2), NodeId(3), NodeId(5)];
    let result = connected_components(graph.node_count(), &selected, |node| {
        Ok::<_, Infallible>(
            graph
                .forward_neighbors(node)
                .unwrap()
                .iter()
                .map(|neighbor| neighbor.node)
                .collect(),
        )
    })
    .unwrap();
    assert_eq!(
        result.components,
        vec![
            vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)],
            vec![NodeId(5)]
        ]
    );
    assert_eq!(result.component_of[0], Some(0));
    assert_eq!(result.component_of[5], Some(1));
}

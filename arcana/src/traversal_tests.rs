use crate::storage::Neighbor;
use crate::traversal::{TraversalError, shortest_path};
use crate::{EdgeKind, NodeId};

fn neighbors(node: NodeId) -> Result<Vec<Neighbor>, &'static str> {
    Ok(match node.0 {
        0 => vec![Neighbor {
            node: NodeId(1),
            kind: EdgeKind(10),
        }],
        1 => vec![Neighbor {
            node: NodeId(2),
            kind: EdgeKind(20),
        }],
        _ => Vec::new(),
    })
}

#[test]
fn shortest_path_returns_nodes_and_edge_kinds() {
    let path = shortest_path(3, NodeId(0), NodeId(2), 2, neighbors)
        .unwrap()
        .unwrap();
    assert_eq!(path.nodes, vec![NodeId(0), NodeId(1), NodeId(2)]);
    assert_eq!(path.kinds, vec![EdgeKind(10), EdgeKind(20)]);
}

#[test]
fn shortest_path_respects_depth_bound() {
    assert_eq!(
        shortest_path(3, NodeId(0), NodeId(2), 1, neighbors).unwrap(),
        None
    );
}

#[test]
fn shortest_path_rejects_invalid_nodes() {
    let error = shortest_path(3, NodeId(3), NodeId(2), 2, neighbors).unwrap_err();
    assert!(matches!(
        error,
        TraversalError::InvalidNode {
            node: NodeId(3),
            node_count: 3
        }
    ));
}

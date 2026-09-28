use super::*;

#[test]
fn record_family_finalizers_consume_overallocated_staging_vectors() {
    let remap = [StringId(0)];

    let mut nodes = Vec::with_capacity(128);
    nodes.push(TempNodeRecord {
        key: crate::repository::NodeKey::from_u64(1),
        external_identity: super::super::Sha256Identity([1; 32]),
        signature_digest: [2; 32],
        content_id: None,
        owner: None,
        path: TempStringId(0),
        name: TempStringId(0),
        qualified_name: TempStringId(0),
        span: None,
        kind_code: 1,
    });
    let node_capacity = nodes.capacity();
    let nodes = finish_nodes(nodes, &remap);
    assert_eq!(nodes.len(), 1);
    assert!(nodes.capacity() < node_capacity);

    let mut edges = Vec::with_capacity(128);
    edges.push(TempEdgeRecord {
        source: crate::repository::NodeKey::from_u64(1),
        target: crate::repository::NodeKey::from_u64(1),
        relation_code: 1,
        span: None,
    });
    let edge_capacity = edges.capacity();
    let edges = finish_edges(edges, &remap);
    assert_eq!(edges.len(), 1);
    assert!(edges.capacity() < edge_capacity);

    let mut unresolved = Vec::with_capacity(128);
    unresolved.push(TempUnresolvedRecord {
        source: crate::repository::NodeKey::from_u64(1),
        relation_code: 1,
        reason_code: 1,
        expression: TempStringId(0),
        candidate_namespace: None,
        candidate_name: None,
        unknown_reason: None,
        span: None,
    });
    let unresolved_capacity = unresolved.capacity();
    let unresolved = finish_unresolved(unresolved, &remap);
    assert_eq!(unresolved.len(), 1);
    assert!(unresolved.capacity() < unresolved_capacity);
}

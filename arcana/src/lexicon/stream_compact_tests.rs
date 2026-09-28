use super::{CompactPass, RecordCounts};
use crate::lexicon::identity::LexiconIdentity;
use crate::lexicon::object::NodeReference;
use crate::repository_store::{Sha256Identity, TempStringId};

#[test]
fn identity_resolution_uses_canonical_staging_not_external_id_map() {
    let mut pass = CompactPass::new();
    let identity = LexiconIdentity::from_digest([7; 32]);
    let key = identity.node_key();
    pass.assembler.push_node(
        key,
        Sha256Identity(identity.digest()),
        [9; 32],
        None,
        None,
        1,
        TempStringId(0),
        TempStringId(0),
        TempStringId(0),
        None,
    );
    pass.assembler.canonicalize_nodes().unwrap();

    let unknown = LexiconIdentity::from_digest([8; 32]);
    pass.external_ids.insert(unknown, key);

    assert_eq!(
        pass.resolve(NodeReference::Identity(identity)).unwrap(),
        key
    );
    assert!(matches!(
        pass.resolve(NodeReference::Identity(unknown)),
        Err(super::LexiconSnapshotError::Malformed(
            "unknown relationship node"
        ))
    ));
    assert!(
        !pass
            .assembler
            .contains_node_identity(key, Sha256Identity(unknown.digest()))
    );
}

#[test]
fn relation_capacity_is_planned_during_nodes_and_reserved_during_relations() {
    let mut pass = CompactPass::new();
    let first = RecordCounts {
        nodes: 3,
        edges: 5,
        unresolved: 7,
    };
    let second = RecordCounts {
        nodes: 2,
        edges: 11,
        unresolved: 13,
    };

    pass.reserve_object(first).unwrap();
    pass.reserve_object(second).unwrap();
    let (nodes, edges, unresolved) = pass.assembler.capacities();
    assert!(nodes >= 5);
    assert_eq!(edges, 0);
    assert_eq!(unresolved, 0);

    pass.finish_node_pass().unwrap();
    let (_, edges, unresolved) = pass.assembler.capacities();
    assert_eq!(edges, 0);
    assert_eq!(unresolved, 0);

    pass.reserve_relation_object(first).unwrap();
    let (_, edges, unresolved) = pass.assembler.capacities();
    assert!(edges >= 5);
    assert!(unresolved >= 7);

    pass.reserve_relation_object(second).unwrap();
    let (_, edges, unresolved) = pass.assembler.capacities();
    assert!(edges >= 16);
    assert!(unresolved >= 20);
}

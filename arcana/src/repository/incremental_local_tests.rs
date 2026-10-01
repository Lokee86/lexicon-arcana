use std::fs;

use crate::repository_store::{
    CompactRepositoryAssembler, RepositoryStoreFile, write_repository_store,
};
use crate::synthetic::{Edge, EdgeKind, NodeId};

use super::incremental_local_test_support::{delta, facts, push_function, temp_path};
use super::*;

#[test]
fn compact_delta_edge_changes_match_full_graph_difference() {
    let base = facts(false);
    let current = facts(true);
    let path = temp_path();
    write_repository_store(&path, &base).unwrap();
    let mut store = RepositoryStoreFile::open(&path).unwrap();
    let delta = delta(true);

    let changes =
        plan_compact_delta_edge_changes_from_store(&mut store, &delta, &["a.go".to_owned()])
            .unwrap();

    let expected = super::incremental_diff::edge_difference(
        &compile_repository_graph(&base).unwrap().dataset.edges,
        &compile_repository_graph(&current).unwrap().dataset.edges,
    );
    assert_eq!(changes, expected);
    assert_eq!(
        changes.removed,
        vec![Edge {
            source: NodeId(1),
            target: NodeId(2),
            kind: EdgeKind(5),
        }]
    );
    assert_eq!(
        changes.added,
        vec![Edge {
            source: NodeId(2),
            target: NodeId(1),
            kind: EdgeKind(5),
        }]
    );

    fs::remove_file(path).unwrap();
}

#[test]
fn compact_delta_edge_changes_remove_old_relationship_without_full_graph() {
    let base = facts(false);
    let path = temp_path();
    write_repository_store(&path, &base).unwrap();
    let mut store = RepositoryStoreFile::open(&path).unwrap();

    let mut assembler = CompactRepositoryAssembler::with_capacity(1, 0, 0);
    push_function(&mut assembler, NodeKey::from_u64(2), "a.go");
    let delta = assembler.finish_delta().unwrap();

    let changes =
        plan_compact_delta_edge_changes_from_store(&mut store, &delta, &["a.go".to_owned()])
            .unwrap();

    assert!(changes.added.is_empty());
    assert_eq!(
        changes.removed,
        vec![Edge {
            source: NodeId(1),
            target: NodeId(2),
            kind: EdgeKind(5),
        }]
    );

    fs::remove_file(path).unwrap();
}

#[test]
fn compact_delta_edge_changes_reject_missing_unchanged_endpoint() {
    let base = facts(false);
    let path = temp_path();
    write_repository_store(&path, &base).unwrap();
    let mut store = RepositoryStoreFile::open(&path).unwrap();

    let mut assembler = CompactRepositoryAssembler::with_capacity(1, 1, 0);
    push_function(&mut assembler, NodeKey::from_u64(2), "a.go");
    assembler.push_edge(
        NodeKey::from_u64(2),
        NodeKey::from_u64(99),
        relation_to_edge_kind(&RelationKind::Calls).0,
        None,
    );
    let delta = assembler.finish_delta().unwrap();

    assert!(matches!(
        plan_compact_delta_edge_changes_from_store(
            &mut store,
            &delta,
            &["a.go".to_owned()],
        ),
        Err(IncrementalError::Compile(RepositoryCompileError::MissingEdgeEndpoint { key }))
            if key == NodeKey::from_u64(99)
    ));

    fs::remove_file(path).unwrap();
}

#[test]
fn compact_delta_edge_changes_reject_changed_node_set() {
    let base = facts(false);
    let path = temp_path();
    write_repository_store(&path, &base).unwrap();
    let mut store = RepositoryStoreFile::open(&path).unwrap();

    let mut assembler = CompactRepositoryAssembler::with_capacity(1, 0, 0);
    push_function(&mut assembler, NodeKey::from_u64(4), "a.go");
    let delta = assembler.finish_delta().unwrap();

    assert!(matches!(
        plan_compact_delta_edge_changes_from_store(
            &mut store,
            &delta,
            &["a.go".to_owned()],
        ),
        Err(IncrementalError::NodeSetChanged { added, removed })
            if added == vec![NodeKey::from_u64(4)]
                && removed == vec![NodeKey::from_u64(2)]
    ));

    fs::remove_file(path).unwrap();
}

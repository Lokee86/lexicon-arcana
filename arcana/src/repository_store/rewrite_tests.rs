use std::fs;

use crate::repository::{NodeKey, NodeKind};

use super::format::node_kind_code;
use super::rewrite_test_support::{delta, facts, moved_delta, moved_facts};
use super::writer_test_support::{cleanup, temp_path};
use super::*;

#[test]
fn streaming_rewrite_is_byte_identical_to_clean_canonical_store() {
    let base = facts(false);
    let current = facts(true);
    let delta = delta();
    let base_path = temp_path("rewrite-base");
    let rewritten_path = temp_path("rewrite-output");
    let clean_path = temp_path("rewrite-clean");

    write_repository_store(&base_path, &base).unwrap();
    let mut base_file = RepositoryStoreFile::open(&base_path).unwrap();
    let rewritten = rewrite_repository_store(
        &rewritten_path,
        &mut base_file,
        &["src/a.rs".to_owned()],
        &delta,
    )
    .unwrap();
    let clean = write_repository_store(&clean_path, &current).unwrap();

    assert_eq!(rewritten.write, clean);
    assert_eq!(
        fs::read(&rewritten_path).unwrap(),
        fs::read(&clean_path).unwrap()
    );
    cleanup(&[&base_path, &rewritten_path, &clean_path]);
}

#[test]
fn streaming_rewrite_moves_owner_path_and_preserves_owner_fallbacks() {
    let base = moved_facts("src/a.rs");
    let current = moved_facts("src/c.rs");
    let delta = moved_delta("src/c.rs");
    let base_path = temp_path("rewrite-move-base");
    let rewritten_path = temp_path("rewrite-move-output");
    let clean_path = temp_path("rewrite-move-clean");

    write_repository_store(&base_path, &base).unwrap();
    let mut base_file = RepositoryStoreFile::open(&base_path).unwrap();
    rewrite_repository_store(
        &rewritten_path,
        &mut base_file,
        &["src/a.rs".to_owned(), "src/c.rs".to_owned()],
        &delta,
    )
    .unwrap();
    write_repository_store(&clean_path, &current).unwrap();

    assert_eq!(
        fs::read(&rewritten_path).unwrap(),
        fs::read(&clean_path).unwrap()
    );
    let mut rewritten = RepositoryStoreFile::open(&rewritten_path).unwrap();
    assert!(
        rewritten
            .owned_facts(&["src/a.rs".to_owned()])
            .unwrap()
            .nodes
            .is_empty()
    );
    let moved = rewritten.owned_facts(&["src/c.rs".to_owned()]).unwrap();
    assert_eq!(moved.nodes.len(), 1);
    assert_eq!(moved.edges.len(), 2);

    cleanup(&[&base_path, &rewritten_path, &clean_path]);
}

#[test]
fn streaming_rewrite_rejects_changed_owned_node_set() {
    let base = facts(false);
    let base_path = temp_path("rewrite-node-set");
    let output = temp_path("rewrite-node-set-output");
    write_repository_store(&base_path, &base).unwrap();
    let mut base_file = RepositoryStoreFile::open(&base_path).unwrap();

    let mut assembler = CompactRepositoryAssembler::with_capacity(1, 0, 0);
    let path = assembler.intern("src/a.rs").unwrap();
    let name = assembler.intern("replacement").unwrap();
    let qualified = assembler.intern("crate::replacement").unwrap();
    assembler.push_node(
        NodeKey::from_u64(99),
        Sha256Identity([99; 32]),
        [99; 32],
        None,
        None,
        node_kind_code(&NodeKind::Function),
        path,
        name,
        qualified,
        None,
    );
    let delta = assembler.finish_delta().unwrap();

    assert!(matches!(
        rewrite_repository_store(&output, &mut base_file, &["src/a.rs".to_owned()], &delta),
        Err(RepositoryStoreWriteError::ReplacementNodeSetMismatch)
    ));
    cleanup(&[&base_path, &output]);
}

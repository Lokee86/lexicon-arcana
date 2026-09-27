use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::repository_store::write_repository_store;

use super::*;

#[test]
fn store_backed_plan_uses_complete_current_snapshot_without_old_fact_materialization() {
    let base = facts(NodeKey::from_u64(2), NodeKey::from_u64(3), false);
    let current = facts(NodeKey::from_u64(2), NodeKey::from_u64(3), true);
    let compiled = compile_repository_facts(&base).unwrap();
    let path = temp_path();
    write_repository_store(&path, &base).unwrap();
    let mut store = crate::repository_store::RepositoryStoreFile::open(&path).unwrap();

    let plan = plan_verified_snapshot_update_from_store(
        &mut store,
        &current,
        &["a.go".to_owned()],
        &compiled.dataset,
    )
    .unwrap();
    let update = plan.finish(current.clone());

    assert_eq!(update.facts, current);
    assert_eq!(update.changes.added.len(), 1);
    assert_eq!(update.changes.removed.len(), 1);
    fs::remove_file(path).unwrap();
}

#[test]
fn store_backed_plan_detects_changed_file_node_identity_changes() {
    let base = facts(NodeKey::from_u64(2), NodeKey::from_u64(3), false);
    let current = facts(NodeKey::from_u64(4), NodeKey::from_u64(3), false);
    let compiled = compile_repository_facts(&base).unwrap();
    let path = temp_path();
    write_repository_store(&path, &base).unwrap();
    let mut store = crate::repository_store::RepositoryStoreFile::open(&path).unwrap();

    assert!(matches!(
        plan_verified_snapshot_update_from_store(
            &mut store,
            &current,
            &["a.go".to_owned()],
            &compiled.dataset,
        ),
        Err(IncrementalError::NodeSetChanged { added, removed })
            if added == vec![NodeKey::from_u64(4)]
                && removed == vec![NodeKey::from_u64(2)]
    ));
    fs::remove_file(path).unwrap();
}

fn facts(first: NodeKey, second: NodeKey, reverse: bool) -> RepositoryFacts {
    let (source, target) = if reverse {
        (second, first)
    } else {
        (first, second)
    };
    RepositoryFacts {
        nodes: vec![
            node(NodeKey::from_u64(1), NodeKind::Repository, "repo"),
            node(first, NodeKind::Function, "a.go"),
            node(second, NodeKind::Function, "b.go"),
        ],
        edges: vec![EdgeFact {
            source,
            target,
            relation: RelationKind::Calls,
            span: Some(SourceSpan::new("a.go", 1, 1, 1, 2).unwrap()),
        }],
        unresolved: Vec::new(),
    }
}

fn node(key: NodeKey, kind: NodeKind, path: &str) -> NodeFact {
    NodeFact {
        key,
        external_identity: None,
        kind,
        path: path.to_owned(),
        name: path.to_owned(),
        qualified_name: path.to_owned(),
        content_id: None,
        span: None,
    }
}

fn temp_path() -> PathBuf {
    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "arcana-incremental-store-{}-{}.arcana",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

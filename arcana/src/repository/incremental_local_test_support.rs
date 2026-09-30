use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::repository_store::{CompactRepositoryAssembler, CompactRepositoryDelta, Sha256Identity};

use super::*;

pub(super) fn facts(reverse: bool) -> RepositoryFacts {
    let (source, target) = if reverse {
        (NodeKey::from_u64(3), NodeKey::from_u64(2))
    } else {
        (NodeKey::from_u64(2), NodeKey::from_u64(3))
    };
    RepositoryFacts {
        nodes: vec![
            node(NodeKey::from_u64(1), NodeKind::Repository, "repo"),
            node(NodeKey::from_u64(2), NodeKind::Function, "a.go"),
            node(NodeKey::from_u64(3), NodeKind::Function, "b.go"),
        ],
        edges: vec![
            EdgeFact {
                source,
                target,
                relation: RelationKind::Calls,
                span: Some(SourceSpan::new("a.go", 1, 1, 1, 2).unwrap()),
            },
            EdgeFact {
                source: NodeKey::from_u64(3),
                target: NodeKey::from_u64(2),
                relation: RelationKind::References,
                span: Some(SourceSpan::new("b.go", 2, 1, 2, 2).unwrap()),
            },
        ],
        unresolved: Vec::new(),
    }
}

pub(super) fn delta(reverse: bool) -> CompactRepositoryDelta {
    let mut assembler = CompactRepositoryAssembler::with_capacity(1, 1, 0);
    push_function(&mut assembler, NodeKey::from_u64(2), "a.go");
    let (source, target) = if reverse {
        (NodeKey::from_u64(3), NodeKey::from_u64(2))
    } else {
        (NodeKey::from_u64(2), NodeKey::from_u64(3))
    };
    assembler.push_edge(
        source,
        target,
        relation_to_edge_kind(&RelationKind::Calls).0,
        None,
    );
    assembler.finish_delta().unwrap()
}

pub(super) fn push_function(assembler: &mut CompactRepositoryAssembler, key: NodeKey, path: &str) {
    let path_id = assembler.intern(path).unwrap();
    assembler.push_node(
        key,
        Sha256Identity([key.as_u64() as u8; 32]),
        None,
        crate::repository_store::format::node_kind_code(&NodeKind::Function),
        path_id,
        path_id,
        path_id,
        None,
    );
}

pub(super) fn temp_path() -> PathBuf {
    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "arcana-incremental-local-{}-{}.arcana",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
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

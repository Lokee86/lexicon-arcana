use crate::repository::{
    EdgeFact, NodeFact, NodeKey, NodeKind, RelationKind, RepositoryFacts, SourceSpan,
    UnresolvedReason, UnresolvedReferenceFact,
};

use super::format::{node_kind_code, relation_code, unresolved_reason_code};
use super::*;

pub(super) fn facts(updated: bool) -> RepositoryFacts {
    let a_name = if updated { "a-new" } else { "a-old" };
    let a_qualified = if updated {
        "crate::a_new"
    } else {
        "crate::a_old"
    };
    let relation = if updated {
        RelationKind::PossibleCalls
    } else {
        RelationKind::Calls
    };
    let expression = if updated {
        "new-expression"
    } else {
        "old-expression"
    };

    RepositoryFacts::with_unresolved(
        vec![
            node(1, NodeKind::Repository, "repo", "repo", "repo", None),
            node(
                2,
                NodeKind::Function,
                "src/a.rs",
                a_name,
                a_qualified,
                Some([2; 32]),
            ),
            node(
                3,
                NodeKind::Function,
                "src/b.rs",
                "b",
                "crate::b",
                Some([3; 32]),
            ),
        ],
        vec![
            EdgeFact {
                source: NodeKey::from_u64(2),
                target: NodeKey::from_u64(3),
                relation,
                span: Some(span("src/a.rs", 4)),
            },
            EdgeFact {
                source: NodeKey::from_u64(3),
                target: NodeKey::from_u64(2),
                relation: RelationKind::References,
                span: Some(span("src/b.rs", 5)),
            },
        ],
        vec![
            UnresolvedReferenceFact {
                source: NodeKey::from_u64(2),
                relation: RelationKind::References,
                expression: expression.to_owned(),
                candidate_namespace: None,
                candidate_name: None,
                reason: UnresolvedReason::MissingTarget,
                span: Some(span("src/a.rs", 6)),
            },
            UnresolvedReferenceFact {
                source: NodeKey::from_u64(3),
                relation: RelationKind::References,
                expression: "unchanged".to_owned(),
                candidate_namespace: None,
                candidate_name: None,
                reason: UnresolvedReason::MissingTarget,
                span: Some(span("src/b.rs", 7)),
            },
        ],
    )
}

pub(super) fn moved_facts(path: &str) -> RepositoryFacts {
    RepositoryFacts::new(
        vec![
            node(1, NodeKind::Repository, "repo", "repo", "repo", None),
            node(
                2,
                NodeKind::Function,
                path,
                "moved",
                "crate::moved",
                Some([2; 32]),
            ),
            node(
                3,
                NodeKind::Function,
                "src/b.rs",
                "b",
                "crate::b",
                Some([3; 32]),
            ),
        ],
        vec![
            EdgeFact {
                source: NodeKey::from_u64(2),
                target: NodeKey::from_u64(3),
                relation: RelationKind::Calls,
                span: None,
            },
            EdgeFact {
                source: NodeKey::from_u64(1),
                target: NodeKey::from_u64(2),
                relation: RelationKind::References,
                span: None,
            },
        ],
    )
}

pub(super) fn moved_delta(path_value: &str) -> CompactRepositoryDelta {
    let mut assembler = CompactRepositoryAssembler::with_capacity(1, 2, 0);
    let path = assembler.intern(path_value).unwrap();
    let name = assembler.intern("moved").unwrap();
    let qualified = assembler.intern("crate::moved").unwrap();
    assembler.push_node(
        NodeKey::from_u64(2),
        Sha256Identity([2; 32]),
        None,
        node_kind_code(&NodeKind::Function),
        path,
        name,
        qualified,
        Some(TempSpan {
            path,
            start_line: 2,
            start_column: 1,
            end_line: 2,
            end_column: 2,
        }),
    );
    assembler.push_edge(
        NodeKey::from_u64(2),
        NodeKey::from_u64(3),
        relation_code(&RelationKind::Calls),
        None,
    );
    assembler.push_edge(
        NodeKey::from_u64(1),
        NodeKey::from_u64(2),
        relation_code(&RelationKind::References),
        None,
    );
    assembler.finish_delta().unwrap()
}

pub(super) fn delta() -> CompactRepositoryDelta {
    let mut assembler = CompactRepositoryAssembler::with_capacity(1, 1, 1);
    let path = assembler.intern("src/a.rs").unwrap();
    let name = assembler.intern("a-new").unwrap();
    let qualified = assembler.intern("crate::a_new").unwrap();
    let expression = assembler.intern("new-expression").unwrap();
    assembler.push_node(
        NodeKey::from_u64(2),
        Sha256Identity([2; 32]),
        None,
        node_kind_code(&NodeKind::Function),
        path,
        name,
        qualified,
        Some(TempSpan {
            path,
            start_line: 2,
            start_column: 1,
            end_line: 2,
            end_column: 2,
        }),
    );
    assembler.push_edge(
        NodeKey::from_u64(2),
        NodeKey::from_u64(3),
        relation_code(&RelationKind::PossibleCalls),
        Some(TempSpan {
            path,
            start_line: 4,
            start_column: 1,
            end_line: 4,
            end_column: 2,
        }),
    );
    assembler.push_unresolved(
        NodeKey::from_u64(2),
        relation_code(&RelationKind::References),
        unresolved_reason_code(&UnresolvedReason::MissingTarget),
        expression,
        None,
        None,
        None,
        Some(TempSpan {
            path,
            start_line: 6,
            start_column: 1,
            end_line: 6,
            end_column: 2,
        }),
    );
    assembler.finish_delta().unwrap()
}

fn node(
    key: u64,
    kind: NodeKind,
    path: &str,
    name: &str,
    qualified_name: &str,
    identity: Option<[u8; 32]>,
) -> NodeFact {
    NodeFact {
        key: NodeKey::from_u64(key),
        external_identity: identity.map(|digest| Sha256Identity(digest).canonical_string()),
        kind,
        path: path.to_owned(),
        name: name.to_owned(),
        qualified_name: qualified_name.to_owned(),
        content_id: None,
        span: (key == 2).then(|| span(path, 2)),
    }
}

fn span(path: &str, line: u32) -> SourceSpan {
    SourceSpan::new(path, line, 1, line, 2).unwrap()
}

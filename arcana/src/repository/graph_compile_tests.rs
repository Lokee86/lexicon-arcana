use crate::repository::{
    EdgeFact, NodeFact, NodeKey, NodeKind, RelationKind, RepositoryFacts, SourceSpan,
};
use crate::repository_store::CompactRepositoryBuild;

use super::{compile_compact_repository_graph, compile_repository_graph};

#[test]
fn compact_graph_matches_rich_graph() {
    let first = node("first", "src/a.rs");
    let second = node("second", "src/b.rs");
    let call = EdgeFact {
        source: first.key,
        target: second.key,
        relation: RelationKind::Calls,
        span: Some(SourceSpan::new("src/a.rs", 1, 1, 1, 4).unwrap()),
    };
    let repeated_call = EdgeFact {
        span: Some(SourceSpan::new("src/a.rs", 9, 1, 9, 4).unwrap()),
        ..call.clone()
    };
    let facts = RepositoryFacts::new(
        vec![second.clone(), first.clone()],
        vec![repeated_call, call],
    );

    let rich = compile_repository_graph(&facts).unwrap();
    let build = CompactRepositoryBuild::from_facts(&facts).unwrap();
    let compact = compile_compact_repository_graph(&build).unwrap();

    assert_eq!(compact, rich);
    assert_eq!(compact.dataset.edges.len(), 1);
    assert_eq!(compact.node_id(first.key).unwrap().0, 0);
    assert_eq!(compact.node_id(second.key).unwrap().0, 1);
}

fn node(identity: &str, path: &str) -> NodeFact {
    let key = NodeKey::from_identity(identity);
    NodeFact {
        key,
        external_identity: None,
        kind: NodeKind::Function,
        path: path.to_owned(),
        name: identity.to_owned(),
        qualified_name: identity.to_owned(),
        content_id: None,
        span: None,
    }
}

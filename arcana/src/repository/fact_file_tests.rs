use super::fact_encode::encode_facts;
use super::fact_file::parse_facts;
use super::{
    ContentId, EdgeFact, NodeFact, NodeKey, NodeKind, RelationKind, RepositoryFacts, SourceSpan,
    UnresolvedReason, UnresolvedReferenceFact,
};

#[test]
fn fact_file_round_trips_and_escapes_fields() {
    let facts = RepositoryFacts {
        nodes: vec![NodeFact {
            key: NodeKey::from_identity("node\t1"),
            external_identity: Some(format!("sha256:{}", "2".repeat(64))),
            kind: NodeKind::Function,
            path: "src/main.rs".to_owned(),
            name: "line\t\nname\\".to_owned(),
            qualified_name: "demo::line\t\nname\\".to_owned(),
            content_id: Some(ContentId::from_bytes(b"body")),
            span: Some(SourceSpan::new("src\\main.rs", 2, 3, 4, 5).unwrap()),
        }],
        edges: vec![EdgeFact {
            source: NodeKey::from_identity("source"),
            target: NodeKey::from_identity("target"),
            relation: RelationKind::References,
            span: None,
        }],
        unresolved: vec![UnresolvedReferenceFact {
            source: NodeKey::from_identity("node\t1"),
            relation: RelationKind::Calls,
            expression: "pkg\tCall".to_owned(),
            candidate_namespace: Some("example.com/pkg".to_owned()),
            candidate_name: Some("Call".to_owned()),
            reason: UnresolvedReason::UnsupportedForm,
            span: Some(SourceSpan::new("src/main.rs", 8, 4, 8, 12).unwrap()),
        }],
    };

    let encoded = encode_facts(&facts);
    assert!(encoded.starts_with("version\t4\n"));
    assert!(encoded.contains("\\t") && encoded.contains("\\n") && encoded.contains("\\\\"));
    assert_eq!(parse_facts(&encoded).unwrap(), facts);
}

#[test]
fn encoding_sorts_records_deterministically() {
    let mut facts = RepositoryFacts::default();
    facts.nodes.push(NodeFact {
        key: NodeKey::from_u64(2),
        external_identity: None,
        kind: NodeKind::File,
        path: "b".to_owned(),
        name: String::new(),
        qualified_name: String::new(),
        content_id: None,
        span: None,
    });
    facts.nodes.push(NodeFact {
        key: NodeKey::from_u64(1),
        external_identity: None,
        kind: NodeKind::File,
        path: "a".to_owned(),
        name: String::new(),
        qualified_name: String::new(),
        content_id: None,
        span: None,
    });
    assert_eq!(
        encode_facts(&facts),
        encode_facts(&RepositoryFacts {
            nodes: facts.nodes.iter().rev().cloned().collect(),
            edges: Vec::new(),
            unresolved: Vec::new(),
        })
    );
}

#[test]
fn parser_accepts_version_one_without_unresolved_records() {
    let input = "version\t1\nN\t0000000000000001\tfile\tsrc/main.rs\tmain\t-\t-\t-\t-\t-\t-\n";
    let facts = parse_facts(input).unwrap();
    assert_eq!(facts.nodes.len(), 1);
    assert!(facts.edges.is_empty());
    assert!(facts.unresolved.is_empty());
}

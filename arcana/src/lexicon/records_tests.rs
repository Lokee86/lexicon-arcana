use super::object::{EdgeRecord, FactRecord, NodeRecord, RecordCounts, UnresolvedRecord};
use super::records::build_repository_facts;
use super::stream_records::NodePass;
use crate::repository::{NodeKind, UnresolvedReason};

fn id(value: char) -> String {
    format!("sha256:{}", value.to_string().repeat(64))
}

fn node(identity: String, kind: &str, name: &str) -> FactRecord {
    FactRecord::Node(NodeRecord {
        attributes: None,
        content_id: None,
        id: identity,
        kind: kind.to_owned(),
        name: name.to_owned(),
        owner: Some("source.cc".to_owned()),
        path: "source.cc".to_owned(),
        qualified_name: format!("demo::{name}"),
        span: None,
    })
}

#[test]
fn bounded_two_pass_builder_matches_monolithic_conversion() {
    let source = id('1');
    let target = id('2');
    let first = vec![
        node(source.clone(), "function", "source"),
        FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner: Some("source.cc".to_owned()),
            relation: "calls".to_owned(),
            source: source.clone(),
            span: None,
            target: target.clone(),
        }),
    ];
    let second = vec![
        node(target.clone(), "future-node-kind", "target"),
        FactRecord::Unresolved(UnresolvedRecord {
            attributes: None,
            candidate_name: Some("target".to_owned()),
            candidate_namespace: None,
            expression: "target()".to_owned(),
            owner: Some("source.cc".to_owned()),
            reason: "future-reason".to_owned(),
            relation: "calls".to_owned(),
            source,
            span: None,
        }),
    ];

    let mut all = first.clone();
    all.extend(second.clone());
    let expected = build_repository_facts(all).unwrap();

    let mut nodes = NodePass::new();
    nodes.ingest(first.clone(), RecordCounts::from_records(&first));
    nodes.ingest(second.clone(), RecordCounts::from_records(&second));
    let mut relations = nodes.finish().unwrap();
    relations.ingest(first);
    relations.ingest(second);
    let actual = relations.finish().unwrap();

    assert_eq!(actual, expected);
}

#[test]
fn accepts_unknown_labels_with_explicit_degradation_warnings() {
    let source = id('1');
    let target = id('2');
    let records = vec![
        node(source.clone(), "future-node-kind", "source"),
        node(target.clone(), "function", "target"),
        FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner: Some("source.cc".to_owned()),
            relation: "future-edge-relation".to_owned(),
            source: source.clone(),
            span: None,
            target,
        }),
        FactRecord::Unresolved(UnresolvedRecord {
            attributes: None,
            candidate_name: None,
            candidate_namespace: None,
            expression: "future()".to_owned(),
            owner: Some("source.cc".to_owned()),
            reason: "future-unresolved-reason".to_owned(),
            relation: "calls".to_owned(),
            source: source.clone(),
            span: None,
        }),
        FactRecord::Unresolved(UnresolvedRecord {
            attributes: None,
            candidate_name: None,
            candidate_namespace: None,
            expression: "ignored()".to_owned(),
            owner: Some("source.cc".to_owned()),
            reason: "missing-target".to_owned(),
            relation: "future-unresolved-relation".to_owned(),
            source,
            span: None,
        }),
    ];

    let (facts, warnings) = build_repository_facts(records).unwrap();
    assert_eq!(facts.nodes.len(), 2);
    assert!(facts.nodes.iter().any(|node| node.kind == NodeKind::Symbol));
    assert!(facts.edges.is_empty());
    assert_eq!(facts.unresolved.len(), 1);
    assert_eq!(
        facts.unresolved[0].reason,
        UnresolvedReason::Unknown("future-unresolved-reason".to_owned())
    );
    for expected in [
        "future-node-kind",
        "future-edge-relation",
        "future-unresolved-reason",
        "future-unresolved-relation",
    ] {
        assert!(warnings.iter().any(|warning| warning.contains(expected)));
    }
}

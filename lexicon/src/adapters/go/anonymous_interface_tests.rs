use std::fs;

use crate::{AdapterRequest, FactRecord, LanguageAdapter};

use super::{
    GoAdapter, identities,
    tests::{TempDirectory, synthetic_helper},
};

#[test]
fn materializes_internal_anonymous_interface_contract_with_provenance() {
    let root = TempDirectory::new("anonymous-interface");
    fs::write(
        root.path.join("go.mod"),
        "module example.com/anon\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(
        root.path.join("main.go"),
        "package anon\nfunc caller() {}\n",
    )
    .unwrap();

    let response = r#"{"protocol_version":2,"observations":[{"observation":"declaration","semantic_key":"package:example.com/anon:anon","kind":"package","name":"anon","owner":"main.go","span":{"start_line":1,"start_column":9,"end_line":1,"end_column":13}},{"observation":"declaration","semantic_key":"function:example.com/anon:caller","kind":"function","name":"caller","owner":"main.go","span":{"start_line":2,"start_column":1,"end_line":2,"end_column":17},"metadata":{"container":"package:example.com/anon:anon"}},{"observation":"callsite","source_key":"function:example.com/anon:caller","form":"interface","resolution":"resolved","targets":[{"semantic_key":"method:example.com/anon:interface{Reindex()}.Reindex","name":"Reindex","namespace":"example.com/anon","container_key":"package:example.com/anon:anon","owner":"main.go","span":{"start_line":2,"start_column":15,"end_line":2,"end_column":15}}],"owner":"main.go","span":{"start_line":2,"start_column":1,"end_line":2,"end_column":17}}]}"#;
    let adapter = GoAdapter::with_frontend(synthetic_helper(&root.path, response));
    let analysis = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository: root.path.clone(),
            ..AdapterRequest::default()
        })
        .unwrap();

    analysis.validate().unwrap();
    let target_identity = "method:example.com/anon:interface{Reindex()}.Reindex";
    let target = identities::node_id(target_identity).unwrap();
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node)
            if node.id == target
                && node.kind == "method"
                && node.name == "Reindex"
                && node.path == "main.go"
                && node.owner.as_deref() == Some("main.go")
                && node.span.as_ref().is_some_and(|span| span.start_line == 2 && span.start_column == 15)
    )));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == "defines" && edge.target == target
    )));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == "calls" && edge.target == target
    )));
}

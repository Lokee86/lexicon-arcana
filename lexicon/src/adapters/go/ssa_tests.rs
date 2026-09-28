use std::fs;

use crate::{AdapterRequest, FactRecord, LanguageAdapter};

use super::{
    GoAdapter, identities,
    tests::{TempDirectory, synthetic_helper},
};

#[test]
fn materializes_synthetic_ssa_function_target() {
    let root = TempDirectory::new("synthetic-ssa");
    fs::write(
        root.path.join("go.mod"),
        "module example.com/ssa\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(root.path.join("main.go"), "package ssa\nfunc caller() {}\n").unwrap();

    let response = r#"{"protocol_version":1,"records":[{"record":"declaration","identity":"package:example.com/ssa:ssa","kind":"package","name":"ssa","owner":"main.go","span":{"start_line":1,"start_column":9,"end_line":1,"end_column":12}},{"record":"declaration","identity":"function:example.com/ssa:caller","kind":"function","name":"caller","owner":"main.go","span":{"start_line":2,"start_column":1,"end_line":2,"end_column":17},"metadata":{"container":"package:example.com/ssa:ssa"}},{"record":"call","source":"function:example.com/ssa:caller","target":"ssa-function:example.com/ssa:(example.com/ssa.Worker).Run$bound:2:1","kind":"definite","class":"dynamic","target_name":"Run$bound","target_namespace":"example.com/ssa","target_container":"package:example.com/ssa:ssa","owner":"main.go","span":{"start_line":2,"start_column":1,"end_line":2,"end_column":17}}]}"#;
    let adapter = GoAdapter::with_frontend(synthetic_helper(&root.path, response));
    let analysis = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository: root.path.clone(),
            ..AdapterRequest::default()
        })
        .unwrap();

    analysis.validate().unwrap();
    let target_identity = "ssa-function:example.com/ssa:(example.com/ssa.Worker).Run$bound:2:1";
    let target = identities::node_id(target_identity).unwrap();
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node)
            if node.id == target
                && node.kind == "function"
                && node.name == "Run$bound"
                && node.path == ".lexicon-repository"
                && node.owner.is_none()
    )));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == "calls" && edge.target == target
    )));
}

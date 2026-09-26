use std::fs;

use crate::{AdapterRequest, LanguageAdapter};

use super::{
    GoAdapter,
    tests::{TempDirectory, synthetic_helper},
};

#[test]
fn rejects_relationship_with_missing_endpoint() {
    let root = TempDirectory::new("missing-relationship");
    fs::write(
        root.path.join("go.mod"),
        "module example.com/relationship\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(
        root.path.join("main.go"),
        "package relationship\ntype A struct{}\n",
    )
    .unwrap();

    let response = r#"{"protocol_version":1,"records":[{"record":"declaration","identity":"package:example.com/relationship:relationship","kind":"package","name":"relationship","owner":"main.go","span":{"start_line":1,"start_column":9,"end_line":1,"end_column":21}},{"record":"declaration","identity":"type:example.com/relationship:A","kind":"type","name":"A","owner":"main.go","span":{"start_line":2,"start_column":6,"end_line":2,"end_column":16},"metadata":{"container":"package:example.com/relationship:relationship"}},{"record":"relationship","source":"type:example.com/relationship:A","target":"type:example.com/relationship:Missing","kind":"extends","owner":"main.go","span":{"start_line":2,"start_column":6,"end_line":2,"end_column":6}}]}"#;
    let adapter = GoAdapter::with_helper(synthetic_helper(&root.path, response));
    let error = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository: root.path.clone(),
            ..AdapterRequest::default()
        })
        .unwrap_err()
        .to_string();

    assert!(error.contains("target is not materialized"), "{error}");
}

#[test]
fn materializes_external_relationship_target_contract() {
    let root = TempDirectory::new("external-relationship");
    fs::write(
        root.path.join("go.mod"),
        "module example.com/external\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(
        root.path.join("main.go"),
        "package external\ntype Wrapped struct{}\n",
    )
    .unwrap();

    let response = r#"{"protocol_version":1,"records":[{"record":"declaration","identity":"package:example.com/external:external","kind":"package","name":"external","owner":"main.go","span":{"start_line":1,"start_column":9,"end_line":1,"end_column":17}},{"record":"declaration","identity":"type:example.com/external:Wrapped","kind":"type","name":"Wrapped","owner":"main.go","span":{"start_line":2,"start_column":6,"end_line":2,"end_column":22},"metadata":{"container":"package:example.com/external:external"}},{"record":"relationship","source":"type:example.com/external:Wrapped","target":"type:io:Reader","kind":"extends","owner":"main.go","span":{"start_line":2,"start_column":6,"end_line":2,"end_column":6}}]}"#;
    let adapter = GoAdapter::with_helper(synthetic_helper(&root.path, response));
    let analysis = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository: root.path.clone(),
            ..AdapterRequest::default()
        })
        .unwrap();

    analysis.validate().unwrap();
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        crate::FactRecord::Node(node)
            if node.kind == "type" && node.name == "Reader" && node.path == "@stdlib/io"
                && node.owner.is_none()
    )));
}

#[test]
fn rejects_implements_self_edge() {
    let root = TempDirectory::new("self-relationship");
    fs::write(
        root.path.join("go.mod"),
        "module example.com/self\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(root.path.join("main.go"), "package self\ntype A struct{}\n").unwrap();

    let response = r#"{"protocol_version":1,"records":[{"record":"declaration","identity":"package:example.com/self:self","kind":"package","name":"self","owner":"main.go","span":{"start_line":1,"start_column":9,"end_line":1,"end_column":13}},{"record":"declaration","identity":"type:example.com/self:A","kind":"type","name":"A","owner":"main.go","span":{"start_line":2,"start_column":6,"end_line":2,"end_column":16},"metadata":{"container":"package:example.com/self:self"}},{"record":"relationship","source":"type:example.com/self:A","target":"type:example.com/self:A","kind":"implements","owner":"main.go","span":{"start_line":2,"start_column":6,"end_line":2,"end_column":6}}]}"#;
    let adapter = GoAdapter::with_helper(synthetic_helper(&root.path, response));
    let error = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository: root.path.clone(),
            ..AdapterRequest::default()
        })
        .unwrap_err()
        .to_string();

    assert!(error.contains("implements self-edge"), "{error}");
}

use std::fs;

use crate::{AdapterRequest, FactRecord, LanguageAdapter};

use super::{
    GoAdapter, identities,
    tests::{TempDirectory, synthetic_helper},
};

#[test]
fn materializes_legacy_data_symbol_without_defines_edge() {
    let root = TempDirectory::new("dataflow");
    fs::write(
        root.path.join("go.mod"),
        "module example.com/dataflow\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(
        root.path.join("main.go"),
        "package dataflow\nfunc run(value int) int { return value }\n",
    )
    .unwrap();

    let target = "variable:example.com/dataflow:main.go:2:10:value";
    let response = format!(
        r#"{{"protocol_version":1,"records":[{{"record":"declaration","identity":"package:example.com/dataflow:dataflow","kind":"package","name":"dataflow","owner":"main.go","span":{{"start_line":1,"start_column":9,"end_line":1,"end_column":17}}}},{{"record":"declaration","identity":"function:example.com/dataflow:run","kind":"function","name":"run","owner":"main.go","span":{{"start_line":2,"start_column":1,"end_line":2,"end_column":41}},"metadata":{{"container":"package:example.com/dataflow:dataflow"}}}},{{"record":"dataflow","source":"function:example.com/dataflow:run","target":"{target}","kind":"read","owner":"main.go","span":{{"start_line":2,"start_column":34,"end_line":2,"end_column":39}}}}]}}"#
    );
    let adapter = GoAdapter::with_helper(synthetic_helper(&root.path, &response));
    let analysis = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository: root.path.clone(),
            ..AdapterRequest::default()
        })
        .unwrap();

    analysis.validate().unwrap();
    let source_id = identities::node_id("function:example.com/dataflow:run").unwrap();
    let target_id = identities::node_id(target).unwrap();
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node)
            if node.id == target_id
                && node.kind == "variable"
                && node.name == "value"
                && node.owner.as_deref() == Some("main.go")
                && node.path == "main.go"
                && node.span.as_ref().is_some_and(|span|
                    span.start_line == 2 && span.start_column == 10 && span.end_column == 15)
    )));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.source == source_id
                && edge.target == target_id
                && edge.relation == "reads"
                && edge.owner.as_deref() == Some("main.go")
    )));
    assert!(!analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge) if edge.target == target_id && edge.relation == "defines"
    )));
}

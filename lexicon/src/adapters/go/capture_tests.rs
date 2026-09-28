use std::fs;

use crate::{AdapterRequest, FactRecord, LanguageAdapter};

use super::{
    GoAdapter, identities,
    tests::{TempDirectory, synthetic_helper},
};

#[test]
fn materializes_positionless_capture_with_legacy_identity() {
    let root = TempDirectory::new("positionless-capture");
    fs::write(
        root.path.join("go.mod"),
        "module example.com/capture\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(
        root.path.join("main.go"),
        "package capture\nfunc caller() { _ = func() {} }\n",
    )
    .unwrap();

    let closure = "closure:example.com/capture:main.go:2:21";
    let response = format!(
        r#"{{"protocol_version":2,"observations":[{{"observation":"declaration","semantic_key":"package:example.com/capture:capture","kind":"package","name":"capture","owner":"main.go","span":{{"start_line":1,"start_column":9,"end_line":1,"end_column":16}}}},{{"observation":"declaration","semantic_key":"function:example.com/capture:caller","kind":"function","name":"caller","owner":"main.go","span":{{"start_line":2,"start_column":1,"end_line":2,"end_column":31}},"metadata":{{"container":"package:example.com/capture:capture"}}}},{{"observation":"declaration","semantic_key":"{closure}","kind":"function","name":"closure@2:21","owner":"main.go","span":{{"start_line":2,"start_column":21,"end_line":2,"end_column":30}},"metadata":{{"container":"function:example.com/capture:caller"}}}},{{"observation":"capture","source_key":"{closure}","target_name":"value","capture_index":0,"owner":"main.go"}}]}}"#
    );
    let adapter = GoAdapter::with_frontend(synthetic_helper(&root.path, &response));
    let analysis = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository: root.path.clone(),
            ..AdapterRequest::default()
        })
        .unwrap();

    analysis.validate().unwrap();
    let closure_id = identities::node_id(closure).unwrap();
    let capture_identity = identities::capture(&closure_id, 0, "value");
    let capture_id = identities::node_id(&capture_identity).unwrap();

    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node)
            if node.id == capture_id
                && node.kind == "variable"
                && node.name == "value"
                && node.path == "main.go"
                && node.owner.is_none()
                && node.span.is_none()
    )));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.source == closure_id
                && edge.target == capture_id
                && edge.relation == "references"
                && edge.owner.as_deref() == Some("main.go")
                && edge.span.is_none()
    )));
}

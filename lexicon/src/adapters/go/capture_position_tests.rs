use std::fs;

use crate::{AdapterRequest, FactRecord, LanguageAdapter};

use super::{
    GoAdapter, identities,
    tests::{TempDirectory, synthetic_helper},
};

#[test]
fn positioned_capture_keeps_legacy_point_span() {
    let root = TempDirectory::new("positioned-capture");
    fs::write(
        root.path.join("go.mod"),
        "module example.com/capture-position\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(
        root.path.join("main.go"),
        "package capture\nfunc caller() { value := 1; _ = func() int { return value } }\n",
    )
    .unwrap();

    let closure = "closure:example.com/capture-position:main.go:2:33";
    let target = "variable:example.com/capture-position:main.go:2:17:value";
    let response = format!(
        r#"{{"protocol_version":2,"observations":[{{"observation":"declaration","semantic_key":"package:example.com/capture-position:capture","kind":"package","name":"capture","owner":"main.go","span":{{"start_line":1,"start_column":9,"end_line":1,"end_column":16}}}},{{"observation":"declaration","semantic_key":"function:example.com/capture-position:caller","kind":"function","name":"caller","owner":"main.go","span":{{"start_line":2,"start_column":1,"end_line":2,"end_column":69}},"metadata":{{"container":"package:example.com/capture-position:capture"}}}},{{"observation":"declaration","semantic_key":"{closure}","kind":"function","name":"closure@2:33","owner":"main.go","span":{{"start_line":2,"start_column":33,"end_line":2,"end_column":68}},"metadata":{{"container":"function:example.com/capture-position:caller"}}}},{{"observation":"capture","source_key":"{closure}","target_key":"{target}","target_name":"value","capture_index":0,"owner":"main.go","span":{{"start_line":2,"start_column":17,"end_line":2,"end_column":17}}}}]}}"#
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
    let target_id = identities::node_id_for_kind(target, "variable").unwrap();
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node)
            if node.id == target_id
                && node.span.as_ref().is_some_and(|span|
                    span.start_line == 2
                        && span.start_column == 17
                        && span.end_line == 2
                        && span.end_column == 17)
    )));
}

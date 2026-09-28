use std::fs;

use crate::{AdapterRequest, FactRecord, LanguageAdapter};

use super::{
    GoAdapter, identities,
    tests::{TempDirectory, synthetic_helper},
};

#[test]
fn rust_owns_manifest_and_local_import_dependencies() {
    let root = TempDirectory::new("dependencies");
    fs::create_dir_all(root.path.join("shared")).unwrap();
    fs::write(
        root.path.join("go.mod"),
        "module example.com/app\nrequire example.com/external v1.2.3\n",
    )
    .unwrap();
    fs::write(
        root.path.join("main.go"),
        "package app\nimport \"example.com/app/shared\"\nfunc Run() {}\n",
    )
    .unwrap();
    fs::write(
        root.path.join("shared").join("shared.go"),
        "package shared\nfunc Helper() {}\n",
    )
    .unwrap();

    let response = r#"{"protocol_version":2,"observations":[{"observation":"declaration","semantic_key":"package:example.com/app:app","kind":"package","name":"app","owner":"main.go","span":{"start_line":1,"start_column":9,"end_line":1,"end_column":12}},{"observation":"declaration","semantic_key":"import:internal:example.com/app/shared","kind":"import","name":"example.com/app/shared","owner":"main.go","span":{"start_line":2,"start_column":8,"end_line":2,"end_column":32},"metadata":{"container":"package:example.com/app:app","import_alias":"","import_class":"internal","import_path":"example.com/app/shared"}},{"observation":"declaration","semantic_key":"function:example.com/app:Run","kind":"function","name":"Run","owner":"main.go","span":{"start_line":3,"start_column":1,"end_line":3,"end_column":14},"metadata":{"container":"package:example.com/app:app"}},{"observation":"declaration","semantic_key":"package:example.com/app/shared:shared","kind":"package","name":"shared","owner":"shared/shared.go","span":{"start_line":1,"start_column":9,"end_line":1,"end_column":15}}]}"#;
    let adapter = GoAdapter::with_frontend(synthetic_helper(&root.path, response));
    let analysis = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository: root.path.clone(),
            ..AdapterRequest::default()
        })
        .unwrap();
    analysis.validate().unwrap();

    let repository = identities::node_id(&identities::repository("example.com/app")).unwrap();
    let external = identities::node_id("package:dependency:go:example.com/external").unwrap();
    let app = identities::node_id("package:example.com/app:app").unwrap();
    let shared = identities::node_id("package:example.com/app/shared:shared").unwrap();

    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node)
            if node.id == external
                && node.kind == "module"
                && node.path == "@dependencies/go/example.com/external"
                && node.attributes.as_ref().is_some_and(|attrs| attrs["dependency"] == true)
    )));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.source == repository
                && edge.target == external
                && edge.relation == "depends-on"
                && edge.owner.is_none()
                && edge.attributes.as_ref().is_some_and(|attrs|
                    attrs["category"] == "runtime" && attrs["constraint"] == "v1.2.3")
    )));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.source == app
                && edge.target == shared
                && edge.relation == "depends-on"
                && edge.owner.as_deref() == Some("main.go")
                && edge.attributes.as_ref().is_some_and(|attrs|
                    attrs["category"] == "local" && attrs["path"] == true)
    )));
}

#[test]
fn blank_and_dot_imports_do_not_create_local_dependencies() {
    let root = TempDirectory::new("dependency-aliases");
    fs::write(root.path.join("go.mod"), "module example.com/app\n").unwrap();
    fs::write(root.path.join("main.go"), "package app\nfunc Run() {}\n").unwrap();

    let response = r#"{"protocol_version":2,"observations":[{"observation":"declaration","semantic_key":"package:example.com/app:app","kind":"package","name":"app","owner":"main.go","span":{"start_line":1,"start_column":9,"end_line":1,"end_column":12}},{"observation":"declaration","semantic_key":"import:internal:example.com/app/shared","kind":"import","name":"example.com/app/shared","owner":"main.go","span":{"start_line":1,"start_column":1,"end_line":1,"end_column":2},"metadata":{"container":"package:example.com/app:app","import_alias":"_","import_class":"internal","import_path":"example.com/app/shared"}},{"observation":"declaration","semantic_key":"package:example.com/app/shared:shared","kind":"package","name":"shared","owner":"main.go","span":{"start_line":1,"start_column":9,"end_line":1,"end_column":12}}]}"#;
    let adapter = GoAdapter::with_frontend(synthetic_helper(&root.path, response));
    let analysis = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository: root.path.clone(),
            ..AdapterRequest::default()
        })
        .unwrap();

    assert!(!analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge) if edge.relation == "depends-on"
    )));
}

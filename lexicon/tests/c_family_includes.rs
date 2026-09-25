mod support;

use std::fs;

use lexicon::{AdapterHost, AdapterRequest, Analysis, FactRecord};
use serde_json::Value;
use support::TestDirectory;

#[test]
fn emits_repository_local_include_nodes_and_edges() {
    let root = TestDirectory::new("c-family-includes-local");
    write(&root, "include/api.h", "int answer(void);\n");
    write(
        &root,
        "src/main.c",
        "#include \"../include/api.h\"\nint run(void) { return answer(); }\n",
    );

    let analysis = analyze(&root);
    let import = import_node(&analysis, "src/main.c", "../include/api.h");
    assert_eq!(
        attr(import, "expression"),
        Some(&Value::String("\"../include/api.h\"".into()))
    );
    assert_eq!(attr(import, "system"), Some(&Value::Bool(false)));
    assert_eq!(
        attr(import, "target"),
        Some(&Value::String("../include/api.h".into()))
    );

    let target = file_node(&analysis, "include/api.h");
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == "includes"
                && edge.source == import.id
                && edge.target == target.id
    )));
    assert!(!has_unresolved_import(&analysis, &import.id));
}

#[test]
fn unresolved_local_and_system_includes_preserve_reason() {
    let root = TestDirectory::new("c-family-includes-unresolved");
    write(
        &root,
        "main.c",
        "#include \"missing.h\"\n#include <stdio.h>\nint run(void) { return 0; }\n",
    );

    let analysis = analyze(&root);
    let local = import_node(&analysis, "main.c", "missing.h");
    let system = import_node(&analysis, "main.c", "stdio.h");
    assert_unresolved_import(&analysis, &local.id, "\"missing.h\"", "missing-target");
    assert_unresolved_import(&analysis, &system.id, "<stdio.h>", "external-target");
}

#[test]
fn unique_basename_fallback_resolves_but_ambiguous_basename_does_not() {
    let unique = TestDirectory::new("c-family-includes-unique");
    write(&unique, "include/api.h", "int answer(void);\n");
    write(&unique, "src/main.c", "#include \"api.h\"\n");
    let unique_analysis = analyze(&unique);
    let import = import_node(&unique_analysis, "src/main.c", "api.h");
    let target = file_node(&unique_analysis, "include/api.h");
    assert!(unique_analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == "includes"
                && edge.source == import.id
                && edge.target == target.id
    )));

    let ambiguous = TestDirectory::new("c-family-includes-ambiguous");
    write(&ambiguous, "a/api.h", "int a(void);\n");
    write(&ambiguous, "b/api.h", "int b(void);\n");
    write(&ambiguous, "src/main.c", "#include \"api.h\"\n");
    let ambiguous_analysis = analyze(&ambiguous);
    let import = import_node(&ambiguous_analysis, "src/main.c", "api.h");
    assert_unresolved_import(
        &ambiguous_analysis,
        &import.id,
        "\"api.h\"",
        "missing-target",
    );
}

#[test]
fn exact_repository_path_precedes_relative_candidate_like_go_oracle() {
    let root = TestDirectory::new("c-family-includes-precedence");
    write(&root, "api.h", "int root_api(void);\n");
    write(&root, "src/api.h", "int local_api(void);\n");
    write(&root, "src/main.c", "#include \"api.h\"\n");

    let analysis = analyze(&root);
    let import = import_node(&analysis, "src/main.c", "api.h");
    let root_target = file_node(&analysis, "api.h");
    let relative_target = file_node(&analysis, "src/api.h");
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == "includes"
                && edge.source == import.id
                && edge.target == root_target.id
    )));
    assert!(!analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == "includes"
                && edge.source == import.id
                && edge.target == relative_target.id
    )));
}

fn analyze(root: &TestDirectory) -> Analysis {
    AdapterHost::new(root.path.join("adapters"))
        .analyze(&AdapterRequest {
            language: "c-family".into(),
            repository: root.path.clone(),
            ..Default::default()
        })
        .unwrap()
}

fn import_node<'a>(analysis: &'a Analysis, path: &str, target: &str) -> &'a lexicon::NodeRecord {
    analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node)
                if node.kind == "import" && node.path == path && node.name == target =>
            {
                Some(node)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing import {path}:{target}"))
}

fn file_node<'a>(analysis: &'a Analysis, path: &str) -> &'a lexicon::NodeRecord {
    analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node) if node.kind == "file" && node.path == path => Some(node),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing file {path}"))
}

fn attr<'a>(node: &'a lexicon::NodeRecord, key: &str) -> Option<&'a Value> {
    node.attributes.as_ref()?.as_object()?.get(key)
}

fn assert_unresolved_import(analysis: &Analysis, source: &str, expression: &str, reason: &str) {
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Unresolved(value)
            if value.source == source
                && value.relation == "imports"
                && value.expression == expression
                && value.reason == reason
    )));
}

fn has_unresolved_import(analysis: &Analysis, source: &str) -> bool {
    analysis.records.iter().any(|record| {
        matches!(
            record,
            FactRecord::Unresolved(value)
                if value.source == source && value.relation == "imports"
        )
    })
}

fn write(root: &TestDirectory, relative: &str, content: &str) {
    let path = root
        .path
        .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

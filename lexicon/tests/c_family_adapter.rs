mod support;

use std::fs;

use lexicon::{AdapterHost, AdapterMode, AdapterRequest, FactRecord};
use serde_json::{Value, json};
use support::TestDirectory;

#[test]
fn discovers_c_family_sources_and_registers_by_default() {
    let root = TestDirectory::new("c-family-discovery");
    write(&root, "src/main.c", "int main(void) { return 0; }\n");
    write(&root, "src/tool.cpp", "int tool() { return 1; }\n");
    write(&root, ".git/ignored.c", "int ignored(void);\n");
    write(&root, "build/ignored.cpp", "int ignored();\n");
    write(&root, "notes.txt", "not source\n");

    let host = AdapterHost::new(root.path.join("adapters"));
    assert!(host.has_adapter("c-family"));
    let analysis = analyze(&host, &root, AdapterMode::Full, vec![]);

    let mut files = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) if node.kind == "file" => Some(node.path.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    files.sort();
    assert_eq!(files, ["src/main.c", "src/tool.cpp"]);
    assert_file_languages(&analysis.records, "src/main.c", "c", "c");
    assert_file_languages(&analysis.records, "src/tool.cpp", "cpp", "cpp");
}

#[test]
fn compile_commands_can_select_cpp_for_c_extension() {
    let root = TestDirectory::new("c-family-compile-command");
    write(&root, "forced.c", "auto answer() { return 42; }\n");
    let directory = root.path.to_string_lossy().replace('\\', "/");
    write_value(
        &root,
        "compile_commands.json",
        &json!([{
            "directory": directory,
            "file": "forced.c",
            "command": "clang++ -x c++ -c forced.c"
        }]),
    );

    let host = AdapterHost::new(root.path.join("adapters"));
    let analysis = analyze(&host, &root, AdapterMode::Full, vec![]);
    assert_file_languages(&analysis.records, "forced.c", "cpp", "cpp");
}

#[test]
fn ambiguous_headers_inherit_language_from_includers() {
    let root = TestDirectory::new("c-family-header-inference");
    write(
        &root,
        "include/c_api.h",
        "static inline int c_answer(void) { return 42; }\n",
    );
    write(
        &root,
        "include/cpp_api.h",
        "static inline int cpp_answer(void) { return 42; }\n",
    );
    write(
        &root,
        "src/main.c",
        "#include \"../include/c_api.h\"\nint main(void) { return c_answer(); }\n",
    );
    write(
        &root,
        "src/main.cpp",
        "#include \"../include/cpp_api.h\"\nint main() { return cpp_answer(); }\n",
    );

    let host = AdapterHost::new(root.path.join("adapters"));
    let analysis = analyze(&host, &root, AdapterMode::Full, vec![]);
    assert_file_languages(&analysis.records, "include/c_api.h", "c", "c");
    assert_file_languages(&analysis.records, "include/cpp_api.h", "cpp", "cpp");
}

#[test]
fn ambiguous_header_uses_better_parser_without_changing_source_language() {
    let root = TestDirectory::new("c-family-parser-fallback");
    write(&root, "lambda.h", "auto make = []() { return 42; };\n");

    let host = AdapterHost::new(root.path.join("adapters"));
    let analysis = analyze(&host, &root, AdapterMode::Full, vec![]);
    assert_file_languages(&analysis.records, "lambda.h", "c", "cpp");

    let attributes = file_attributes(&analysis.records, "lambda.h");
    assert_ne!(attributes.get("parse_error"), Some(&Value::Bool(true)));
}

#[test]
fn parser_errors_remain_explicit_unresolved_evidence() {
    let root = TestDirectory::new("c-family-parse-error");
    write(&root, "broken.c", "int main( {\n");

    let host = AdapterHost::new(root.path.join("adapters"));
    let analysis = analyze(&host, &root, AdapterMode::Full, vec![]);
    let attributes = file_attributes(&analysis.records, "broken.c");
    assert_eq!(attributes.get("parse_error"), Some(&Value::Bool(true)));

    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Unresolved(value)
            if value.owner.as_deref() == Some("broken.c")
                && value.relation == "references"
                && value.reason == "unsupported-form"
                && value.expression == "broken.c"
    )));
}

#[test]
fn incremental_analysis_keeps_native_scope_contract() {
    let root = TestDirectory::new("c-family-incremental");
    write(&root, "a.c", "int a(void) { return 1; }\n");
    write(&root, "b.cpp", "int b() { return 2; }\n");

    let host = AdapterHost::new(root.path.join("adapters"));
    let analysis = analyze(&host, &root, AdapterMode::Incremental, vec!["a.c".into()]);

    assert_eq!(analysis.header.mode.as_deref(), Some("incremental"));
    assert_eq!(analysis.header.changed_files, Some(vec!["a.c".into()]));
    assert_eq!(analysis.header.shared_complete, Some(false));
    assert!(analysis.records.iter().all(|record| match record {
        FactRecord::Node(value) => value.owner.as_deref() == Some("a.c"),
        FactRecord::Edge(value) => value.owner.as_deref() == Some("a.c"),
        FactRecord::Unresolved(value) => value.owner.as_deref() == Some("a.c"),
    }));
}

fn analyze(
    host: &AdapterHost,
    root: &TestDirectory,
    mode: AdapterMode,
    changed_files: Vec<String>,
) -> lexicon::Analysis {
    host.analyze(&AdapterRequest {
        language: "c-family".into(),
        mode,
        repository: root.path.clone(),
        changed_files,
        removed_files: Vec::new(),
        ..Default::default()
    })
    .unwrap()
}

fn assert_file_languages(
    records: &[FactRecord],
    path: &str,
    language: &str,
    parser_language: &str,
) {
    let attributes = file_attributes(records, path);
    assert_eq!(
        attributes.get("language").and_then(Value::as_str),
        Some(language)
    );
    assert_eq!(
        attributes.get("parser").and_then(Value::as_str),
        Some("tree-sitter")
    );
    assert_eq!(
        attributes.get("parser_language").and_then(Value::as_str),
        Some(parser_language)
    );
}

fn file_attributes<'a>(
    records: &'a [FactRecord],
    path: &str,
) -> &'a serde_json::Map<String, Value> {
    records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node) if node.kind == "file" && node.path == path => {
                node.attributes.as_ref()?.as_object()
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing file node {path}"))
}

fn write(root: &TestDirectory, relative: &str, content: &str) {
    let path = root
        .path
        .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn write_value(root: &TestDirectory, relative: &str, value: &Value) {
    write(
        root,
        relative,
        &serde_json::to_string_pretty(value).unwrap(),
    );
}

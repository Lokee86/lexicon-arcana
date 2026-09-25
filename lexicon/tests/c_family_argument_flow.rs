mod support;

use std::fs;

use lexicon::{AdapterHost, AdapterRequest, Analysis, FactRecord};
use serde_json::Value;
use support::TestDirectory;

#[test]
fn direct_arguments_pass_parameters_and_locals_to_callee_parameters() {
    let root = TestDirectory::new("c-family-argument-direct");
    write(
        &root,
        "main.c",
        r#"int sink(int target, int other) { return target + other; }
int run(int value, int second) {
  int local = value;
  return sink(local, second);
}
"#,
    );

    let analysis = analyze(&root);
    assert_pass(&analysis, "run::local", "sink::target", "local", 0, "sink");
    assert_pass(&analysis, "run::second", "sink::other", "second", 1, "sink");
}

#[test]
fn owned_fields_pass_to_parameters_when_no_local_shadows_them() {
    let root = TestDirectory::new("c-family-argument-field");
    write(
        &root,
        "main.cpp",
        r#"int sink(int target) { return target; }
struct Holder {
  int input;
  int run() { return sink(input); }
};
"#,
    );

    let analysis = analyze(&root);
    assert_pass(
        &analysis,
        "Holder::input",
        "sink::target",
        "input",
        0,
        "sink",
    );
}

#[test]
fn caller_parameter_shadows_same_named_field() {
    let root = TestDirectory::new("c-family-argument-shadow");
    write(
        &root,
        "main.cpp",
        r#"int sink(int target) { return target; }
struct Holder {
  int value;
  int run(int value) { return sink(value); }
};
"#,
    );

    let analysis = analyze(&root);
    assert_pass(
        &analysis,
        "Holder::run::value",
        "sink::target",
        "value",
        0,
        "sink",
    );
    assert_no_pass(&analysis, "Holder::value", "sink::target");
}

#[test]
fn unsupported_argument_expressions_do_not_invent_flow() {
    let root = TestDirectory::new("c-family-argument-expression");
    write(
        &root,
        "main.c",
        r#"int sink(int target) { return target; }
int run(int value) { return sink(value + 1); }
"#,
    );

    let analysis = analyze(&root);
    assert_no_pass(&analysis, "run::value", "sink::target");
}

#[test]
fn macro_substitution_preserves_argument_flow() {
    let root = TestDirectory::new("c-family-argument-macro");
    write(
        &root,
        "main.c",
        r#"int sink(int target) { return target; }
#define FORWARD(value) sink(value)
int run(int input) { return FORWARD(input); }
"#,
    );

    let analysis = analyze(&root);
    assert_pass(
        &analysis,
        "run::input",
        "sink::target",
        "(input)",
        0,
        "sink",
    );
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

fn assert_pass(
    analysis: &Analysis,
    source_qualified: &str,
    target_qualified: &str,
    expression: &str,
    argument_index: u64,
    callable: &str,
) {
    let source = node(analysis, source_qualified);
    let target = node(analysis, target_qualified);
    let callable = analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node) if node.name == callable && node.kind != "parameter" => {
                Some(node)
            }
            _ => None,
        })
        .unwrap();
    let edge = analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Edge(edge)
                if edge.relation == "passes-to"
                    && edge.source == source.id
                    && edge.target == target.id =>
            {
                Some(edge)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing passes-to {source_qualified} -> {target_qualified}"));
    let attributes = edge.attributes.as_ref().and_then(Value::as_object).unwrap();
    assert_eq!(
        attributes.get("argument_index").and_then(Value::as_u64),
        Some(argument_index)
    );
    assert_eq!(
        attributes.get("expression").and_then(Value::as_str),
        Some(expression)
    );
    assert_eq!(
        attributes.get("via_call").and_then(Value::as_str),
        Some(callable.id.as_str())
    );
}

fn assert_no_pass(analysis: &Analysis, source_qualified: &str, target_qualified: &str) {
    let source = node(analysis, source_qualified);
    let target = node(analysis, target_qualified);
    assert!(!analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == "passes-to"
                && edge.source == source.id
                && edge.target == target.id
    )));
}

fn node<'a>(analysis: &'a Analysis, qualified: &str) -> &'a lexicon::NodeRecord {
    analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node) if node.qualified_name == qualified => Some(node),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing node {qualified}"))
}

fn write(root: &TestDirectory, relative: &str, content: &str) {
    let path = root
        .path
        .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

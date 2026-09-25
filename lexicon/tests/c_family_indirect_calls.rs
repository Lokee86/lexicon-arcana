mod support;

use std::fs;

use lexicon::{AdapterHost, AdapterRequest, Analysis, FactRecord};
use serde_json::Value;
use support::TestDirectory;

#[test]
fn unrelated_pointer_field_does_not_capture_direct_call() {
    let root = TestDirectory::new("c-family-pointer-unrelated");
    write(
        &root,
        "callback.h",
        "struct callbacks { int (*close)(int value); };\n",
    );
    write(
        &root,
        "main.c",
        "#include \"callback.h\"\nint run(void) { return close(1); }\n",
    );

    let analysis = analyze(&root);
    assert_unresolved(&analysis, "run", "close", "external-target");
}

#[test]
fn callback_arguments_emit_possible_calls_and_dynamic_evidence() {
    let root = TestDirectory::new("c-family-pointer-callback");
    write(
        &root,
        "callback.c",
        r#"int increment(int value) { return value + 1; }
int apply(int (*callback)(int), int value) { return callback(value); }
int run(void) { return apply(increment, 41); }
"#,
    );

    let analysis = analyze(&root);
    assert_pointer_call(&analysis, "apply", "increment");
    assert_unresolved(&analysis, "apply", "callback", "dynamic-target");
}

#[test]
fn initialized_function_pointer_emits_possible_call() {
    let root = TestDirectory::new("c-family-pointer-initializer");
    write(
        &root,
        "callback.c",
        r#"int increment(int value) { return value + 1; }
int run(void) { int (*callback)(int) = increment; return callback(41); }
"#,
    );

    let analysis = analyze(&root);
    assert_pointer_call(&analysis, "run", "increment");
    assert_unresolved(&analysis, "run", "callback", "dynamic-target");
}

#[test]
fn typedef_function_pointer_parameter_emits_possible_call() {
    let root = TestDirectory::new("c-family-pointer-typedef");
    write(
        &root,
        "callback.c",
        r#"typedef int (*callback_fn)(int value);
int increment(int value) { return value + 1; }
int apply(callback_fn callback, int value) { return callback(value); }
int run(void) { return apply(increment, 41); }
"#,
    );

    let analysis = analyze(&root);
    assert_pointer_call(&analysis, "apply", "increment");
}

#[test]
fn designated_function_pointer_initializer_emits_possible_call() {
    let root = TestDirectory::new("c-family-pointer-designated");
    write(
        &root,
        "callback.c",
        r#"typedef int callback_fn(int value);
struct callbacks { callback_fn *run; };
int increment(int value) { return value + 1; }
static struct callbacks callbacks = { .run = increment };
int apply(void) { return callbacks.run(41); }
"#,
    );

    let analysis = analyze(&root);
    assert_pointer_call(&analysis, "apply", "increment");
}

#[test]
fn assigned_function_pointer_field_emits_possible_call() {
    let root = TestDirectory::new("c-family-pointer-field-assignment");
    write(
        &root,
        "callback.c",
        r#"struct callbacks { int (*run)(int value); };
int increment(int value) { return value + 1; }
int apply(struct callbacks *callbacks) { callbacks->run = increment; return callbacks->run(41); }
"#,
    );

    let analysis = analyze(&root);
    assert_pointer_call(&analysis, "apply", "increment");
}

#[test]
fn unbound_function_pointer_call_remains_dynamic() {
    let root = TestDirectory::new("c-family-pointer-dynamic");
    write(
        &root,
        "callback.c",
        "int apply(int (*callback)(int), int value) { return callback(value); }\n",
    );

    let analysis = analyze(&root);
    assert_unresolved(&analysis, "apply", "callback", "dynamic-target");
    assert_no_possible_call(&analysis, "apply");
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

fn assert_pointer_call(analysis: &Analysis, source_name: &str, target_name: &str) {
    let source = node(analysis, source_name);
    let target = node(analysis, target_name);
    let edge = analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Edge(edge)
                if edge.relation == "possible-calls"
                    && edge.source == source.id
                    && edge.target == target.id =>
            {
                Some(edge)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing possible-calls {source_name} -> {target_name}"));
    let attributes = edge.attributes.as_ref().and_then(Value::as_object).unwrap();
    assert_eq!(
        attributes.get("resolution").and_then(Value::as_str),
        Some("possible")
    );
    assert_eq!(
        attributes.get("indirect").and_then(Value::as_str),
        Some("function-pointer")
    );
    assert_eq!(
        attributes.get("candidate_count").and_then(Value::as_u64),
        Some(1)
    );
    let evidence = attributes
        .get("evidence")
        .and_then(Value::as_array)
        .unwrap();
    assert!(
        evidence
            .iter()
            .any(|value| value.as_str() == Some("function-pointer"))
    );
    assert!(
        attributes
            .get("via")
            .and_then(Value::as_array)
            .is_some_and(|values| !values.is_empty())
    );
}

fn assert_unresolved(analysis: &Analysis, source_name: &str, expression: &str, reason: &str) {
    let source = node(analysis, source_name);
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Unresolved(value)
            if value.relation == "calls"
                && value.source == source.id
                && value.expression == expression
                && value.reason == reason
    )));
}

fn assert_no_possible_call(analysis: &Analysis, source_name: &str) {
    let source = node(analysis, source_name);
    assert!(!analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == "possible-calls" && edge.source == source.id
    )));
}

fn node<'a>(analysis: &'a Analysis, name: &str) -> &'a lexicon::NodeRecord {
    analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node) if node.name == name => Some(node),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing node {name}"))
}

fn write(root: &TestDirectory, relative: &str, content: &str) {
    let path = root
        .path
        .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

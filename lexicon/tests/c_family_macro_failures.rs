mod support;

use std::fs;

use lexicon::{AdapterHost, AdapterRequest, Analysis, FactRecord};
use serde_json::Value;
use support::TestDirectory;

#[test]
fn unsupported_macro_forms_remain_explicit() {
    for (name, definition, invocation) in [
        (
            "paste",
            "#define PASTE(name) invoke_##name()",
            "PASTE(task)",
        ),
        ("stringify", "#define SHOW(value) log(#value)", "SHOW(task)"),
        (
            "variadic",
            "#define LOG(format, ...) log(format, __VA_ARGS__)",
            "LOG(\"x\", 1)",
        ),
    ] {
        let root = TestDirectory::new(&format!("c-family-macro-{name}"));
        write(
            &root,
            "main.c",
            &format!("{definition}\nint run(void) {{ return {invocation}; }}\n"),
        );
        let analysis = analyze(&root);
        let unresolved = unresolved(&analysis, "unsupported-macro-expansion");
        let attributes = unresolved
            .attributes
            .as_ref()
            .and_then(Value::as_object)
            .unwrap();
        assert_eq!(
            attributes.get("expansion_depth").and_then(Value::as_u64),
            Some(0)
        );
    }
}

#[test]
fn recursive_and_wrong_arity_macros_stop_explicitly() {
    let root = TestDirectory::new("c-family-macro-cycle");
    write(
        &root,
        "main.c",
        r#"#define FIRST(value) SECOND(value)
#define SECOND(value) FIRST(value)
int run(int input) { return FIRST(input); }
"#,
    );
    let analysis = analyze(&root);
    unresolved(&analysis, "macro-expansion-cycle");

    let root = TestDirectory::new("c-family-macro-arity");
    write(
        &root,
        "main.c",
        "#define PAIR(left, right) target(left, right)\nint run(void) { return PAIR(1); }\n",
    );
    let analysis = analyze(&root);
    unresolved(&analysis, "macro-argument-mismatch");
}

#[test]
fn expansion_depth_is_bounded_explicitly() {
    let root = TestDirectory::new("c-family-macro-depth");
    write(
        &root,
        "main.c",
        r#"#define M0(value) M1(value)
#define M1(value) M2(value)
#define M2(value) M3(value)
#define M3(value) M4(value)
#define M4(value) M5(value)
#define M5(value) M6(value)
#define M6(value) M7(value)
#define M7(value) M8(value)
#define M8(value) M9(value)
#define M9(value) target(value)
int target(int value) { return value; }
int run(int input) { return M0(input); }
"#,
    );
    let analysis = analyze(&root);
    unresolved(&analysis, "macro-expansion-depth");
}

#[test]
fn macro_mediated_pointer_call_reuses_indirect_flow() {
    let root = TestDirectory::new("c-family-macro-pointer");
    write(
        &root,
        "main.c",
        r#"int increment(int value) { return value + 1; }
int apply(int (*callback)(int), int value) {
#define INVOKE(cb, arg) cb(arg)
  return INVOKE(callback, value);
}
int run(void) { return apply(increment, 41); }
"#,
    );

    let analysis = analyze(&root);
    let source = named_node(&analysis, "apply");
    let target = named_node(&analysis, "increment");
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
        .expect("missing macro-mediated function-pointer target");
    let attributes = edge.attributes.as_ref().and_then(Value::as_object).unwrap();
    assert_eq!(
        attributes.get("indirect").and_then(Value::as_str),
        Some("macro")
    );
    let evidence = attributes
        .get("evidence")
        .and_then(Value::as_array)
        .unwrap();
    assert!(
        evidence
            .iter()
            .any(|value| value.as_str() == Some("macro-body"))
    );
    assert!(
        evidence
            .iter()
            .any(|value| value.as_str() == Some("function-pointer"))
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

fn unresolved<'a>(analysis: &'a Analysis, reason: &str) -> &'a lexicon::UnresolvedRecord {
    analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Unresolved(value) if value.reason == reason => Some(value),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing unresolved {reason}"))
}

fn named_node<'a>(analysis: &'a Analysis, name: &str) -> &'a lexicon::NodeRecord {
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

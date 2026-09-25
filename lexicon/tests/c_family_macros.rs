mod support;

use std::fs;

use lexicon::{AdapterHost, AdapterRequest, Analysis, FactRecord};
use serde_json::Value;
use support::TestDirectory;

#[test]
fn function_like_macro_emits_reference_without_fabricating_a_call() {
    let root = TestDirectory::new("c-family-macro-reference");
    write(&root, "macro.h", "#define APPLY(value) ((value) + 1)\n");
    write(
        &root,
        "main.c",
        "#include \"macro.h\"\nint run(void) { return APPLY(41); }\n",
    );

    let analysis = analyze(&root);
    assert_edge(&analysis, "references", "run", "APPLY");
    assert!(!analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Unresolved(value)
            if value.relation == "calls" && value.expression.contains("APPLY")
    )));
}

#[test]
fn aliases_wrappers_and_conditional_macros_resolve_with_macro_provenance() {
    let root = TestDirectory::new("c-family-macro-aliases");
    write(
        &root,
        "api.h",
        r#"#ifndef API_H
#define API_H
int target(int value);
#define WRAP(value) target(value)
#define ALIAS target
#ifdef PLATFORM
#define MAYBE target
#endif
#endif
"#,
    );
    write(
        &root,
        "main.c",
        "#include \"api.h\"\nint run(void) { return WRAP(1) + ALIAS(2) + MAYBE(3); }\n",
    );
    write(
        &root,
        "target.c",
        "int target(int value) { return value; }\n",
    );

    let analysis = analyze(&root);
    assert_macro_call(&analysis, "calls", "run", "target", "macro-body", 0);
    assert_macro_call(&analysis, "calls", "run", "target", "macro-alias", 0);
    assert_macro_call(
        &analysis,
        "possible-calls",
        "run",
        "target",
        "macro-alias",
        0,
    );
    for macro_name in ["WRAP", "ALIAS", "MAYBE"] {
        assert_edge(&analysis, "references", "run", macro_name);
    }
}

#[test]
fn nested_expansion_keeps_complete_chain_and_substitution_provenance() {
    let root = TestDirectory::new("c-family-macro-nested");
    write(
        &root,
        "main.c",
        r#"int sink(int target) { return target; }
#define INNER(value) sink(value)
#define OUTER(value) INNER(value)
int run(int input) { return OUTER(input); }
"#,
    );

    let analysis = analyze(&root);
    let edge = find_edge(&analysis, "calls", "run", "sink");
    let attributes = edge.attributes.as_ref().and_then(Value::as_object).unwrap();
    assert_eq!(
        attributes.get("expansion_depth").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        attributes.get("indirect").and_then(Value::as_str),
        Some("macro")
    );
    assert_eq!(
        attributes
            .get("via")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    let substitutions = attributes
        .get("substitutions")
        .and_then(Value::as_object)
        .unwrap();
    assert_eq!(
        substitutions.get("value").and_then(Value::as_str),
        Some("(input)")
    );
    assert_evidence(attributes, "macro-body");
    assert_evidence(attributes, "argument-substitution");
}

#[test]
fn macro_expansion_emits_every_body_call_in_ordered_body_model() {
    let root = TestDirectory::new("c-family-macro-multiple");
    write(
        &root,
        "main.c",
        r#"int first(int target) { return target; }
int second(int target) { return target; }
#define BOTH(value) first(value); second(value)
int run(int input) { BOTH(input); return input; }
"#,
    );

    let analysis = analyze(&root);
    let first = find_edge(&analysis, "calls", "run", "first");
    let second = find_edge(&analysis, "calls", "run", "second");
    assert_eq!(
        first
            .attributes
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|values| values.get("macro_call_index"))
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        second
            .attributes
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|values| values.get("macro_call_index"))
            .and_then(Value::as_u64),
        Some(1)
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

fn assert_macro_call(
    analysis: &Analysis,
    relation: &str,
    source: &str,
    target: &str,
    evidence: &str,
    depth: u64,
) {
    let edge = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Edge(edge)
                if edge.relation == relation
                    && node(analysis, &edge.source).is_some_and(|value| value.name == source)
                    && node(analysis, &edge.target).is_some_and(|value| value.name == target) =>
            {
                Some(edge)
            }
            _ => None,
        })
        .find(|edge| {
            edge.attributes
                .as_ref()
                .and_then(Value::as_object)
                .is_some_and(|attributes| {
                    attributes
                        .get("evidence")
                        .and_then(Value::as_array)
                        .is_some_and(|values| {
                            values.iter().any(|value| value.as_str() == Some(evidence))
                        })
                        && attributes.get("expansion_depth").and_then(Value::as_u64) == Some(depth)
                })
        })
        .unwrap_or_else(|| panic!("missing {relation} {source} -> {target} with {evidence}"));
    assert_eq!(
        edge.attributes
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|attributes| attributes.get("indirect"))
            .and_then(Value::as_str),
        Some("macro")
    );
}

fn assert_edge(analysis: &Analysis, relation: &str, source: &str, target: &str) {
    let _ = find_edge(analysis, relation, source, target);
}

fn find_edge<'a>(
    analysis: &'a Analysis,
    relation: &str,
    source: &str,
    target: &str,
) -> &'a lexicon::EdgeRecord {
    analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Edge(edge)
                if edge.relation == relation
                    && node(analysis, &edge.source).is_some_and(|value| value.name == source)
                    && node(analysis, &edge.target).is_some_and(|value| value.name == target) =>
            {
                Some(edge)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing {relation} {source} -> {target}"))
}

fn node<'a>(analysis: &'a Analysis, id: &str) -> Option<&'a lexicon::NodeRecord> {
    analysis.records.iter().find_map(|record| match record {
        FactRecord::Node(node) if node.id == id => Some(node),
        _ => None,
    })
}

fn assert_evidence(attributes: &serde_json::Map<String, Value>, evidence: &str) {
    assert!(
        attributes
            .get("evidence")
            .and_then(Value::as_array)
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(evidence)))
    );
}

fn write(root: &TestDirectory, relative: &str, content: &str) {
    let path = root
        .path
        .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

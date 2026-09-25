mod support;

use std::fs;

use lexicon::{AdapterHost, AdapterRequest, Analysis, FactRecord};
use serde_json::Value;
use support::TestDirectory;

#[test]
fn qualified_calls_distinguish_namespaces_types_and_template_types() {
    let root = TestDirectory::new("c-family-calls-qualified");
    write(
        &root,
        "main.cpp",
        r#"namespace api { int run(int value) { return value + 1; } }
class Worker { public: static int run(int value) { return value + 2; } };
template <typename T> class Holder { public: static int get() { return 3; } };
int run(int value) { return value + 4; }
int namespace_call() { return api::run(1); }
int type_call() { return Worker::run(1); }
int template_call() { return Holder<int>::get(); }
"#,
    );

    let analysis = analyze(&root);
    assert_call(
        &analysis,
        "namespace_call",
        "api::run",
        "explicit-qualification",
    );
    assert_call(
        &analysis,
        "type_call",
        "Worker::run",
        "enclosing-type-ownership",
    );
    assert_call(
        &analysis,
        "template_call",
        "Holder::get",
        "enclosing-type-ownership",
    );
}

#[test]
fn arity_pruning_produces_definite_calls_without_guessing_all_incompatible_candidates() {
    let root = TestDirectory::new("c-family-calls-arity");
    write(
        &root,
        "main.cpp",
        r#"int select(int value) { return value; }
int select(int first, int second) { return first + second; }
int route(int first, ...) { return first; }
int route(int first, int second) { return first + second; }
int one() { return select(1); }
int many() { return route(1, 2, 3); }
"#,
    );

    let analysis = analyze(&root);
    assert_call_with_parameter_count(&analysis, "one", "select", 1);
    assert_call_with_parameter_count(&analysis, "many", "route", 1);
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

fn assert_call(
    analysis: &Analysis,
    source_qualified: &str,
    target_qualified: &str,
    evidence: &str,
) {
    let source = node(analysis, source_qualified);
    let target = node(analysis, target_qualified);
    let edge = analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Edge(edge)
                if edge.relation == "calls"
                    && edge.source == source.id
                    && edge.target == target.id =>
            {
                Some(edge)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing call {source_qualified} -> {target_qualified}"));
    let attributes = edge.attributes.as_ref().and_then(Value::as_object).unwrap();
    let values = attributes
        .get("evidence")
        .and_then(Value::as_array)
        .unwrap();
    assert!(values.iter().any(|value| value.as_str() == Some(evidence)));
    assert_eq!(
        attributes.get("resolution").and_then(Value::as_str),
        Some("definite")
    );
}

fn assert_call_with_parameter_count(
    analysis: &Analysis,
    source: &str,
    target_name: &str,
    parameter_count: u64,
) {
    let source_node = node(analysis, source);
    let edge = analysis
        .records
        .iter()
        .find_map(|record| match record {
            FactRecord::Edge(edge) if edge.relation == "calls" && edge.source == source_node.id => {
                let target = node_by_id(analysis, &edge.target)?;
                (target.name == target_name
                    && target
                        .attributes
                        .as_ref()
                        .and_then(Value::as_object)
                        .and_then(|attributes| attributes.get("parameter_count"))
                        .and_then(Value::as_u64)
                        == Some(parameter_count))
                .then_some(edge)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing resolved {source} -> {target_name}/{parameter_count}"));
    let evidence = edge
        .attributes
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|attributes| attributes.get("evidence"))
        .and_then(Value::as_array)
        .unwrap();
    assert!(
        evidence
            .iter()
            .any(|value| value.as_str() == Some("arity-pruning"))
    );
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

fn node_by_id<'a>(analysis: &'a Analysis, id: &str) -> Option<&'a lexicon::NodeRecord> {
    analysis.records.iter().find_map(|record| match record {
        FactRecord::Node(node) if node.id == id => Some(node),
        _ => None,
    })
}

fn write(root: &TestDirectory, relative: &str, content: &str) {
    let path = root
        .path
        .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

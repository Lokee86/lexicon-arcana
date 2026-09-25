use std::collections::BTreeSet;
use std::path::PathBuf;

use lexicon::adapters::rust::RustAdapter;
use lexicon::{AdapterHost, AdapterRequest, FactRecord, LanguageAdapter};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("adapters")
        .join("rust")
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn analyze(name: &str) -> lexicon::Analysis {
    let mut analysis = RustAdapter
        .analyze(&AdapterRequest {
            language: "rust".into(),
            repository: fixture(name),
            ..Default::default()
        })
        .unwrap();
    analysis.canonicalize().unwrap();
    analysis.validate().unwrap();
    analysis
}

#[test]
fn rust_is_registered_by_default_with_native_fingerprint() {
    let host = AdapterHost::new(fixture("sample").join("unused-adapters"));
    assert!(host.has_adapter("rust"));
    assert!(host.fingerprint("rust").unwrap().starts_with("sha256:"));
}

#[test]
fn rust_sample_preserves_calls_relationships_dataflow_and_unresolved_semantics() {
    let analysis = analyze("sample");

    for (source, target, relation) in [
        (
            "lexicon_fixture::lexicon_fixture::Service::run_local",
            "lexicon_fixture::lexicon_fixture::child::Worker::work",
            "calls",
        ),
        (
            "lexicon_fixture::lexicon_fixture::Service::Runnable::run",
            "lexicon_fixture::lexicon_fixture::Runnable::run",
            "overrides",
        ),
        (
            "lexicon_fixture::lexicon_fixture::flow",
            "lexicon_fixture::lexicon_fixture::FLOW_CONST",
            "reads",
        ),
    ] {
        assert_edge(&analysis.records, source, target, relation);
    }

    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge) if edge.relation == "implements"
    )));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == "depends-on"
                && edge.attributes.as_ref().and_then(|value| value.get("path"))
                    == Some(&serde_json::json!(true))
    )));
    for reason in [
        "missing-target",
        "external-target",
        "builtin-target",
        "generated-target",
        "dynamic-target",
    ] {
        assert!(
            analysis.records.iter().any(|record| matches!(
                record,
                FactRecord::Unresolved(value)
                    if value.relation == "calls" && value.reason == reason
            )),
            "missing unresolved reason {reason}"
        );
    }
}

#[test]
fn rust_semantic_fixture_preserves_protocol_facts() {
    let analysis = analyze("semantic_fixture");
    let nodes = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) => Some(node),
            _ => None,
        })
        .collect::<Vec<_>>();

    let handlers = nodes
        .iter()
        .filter(|node| node.name == "error-handler:rust")
        .count();
    assert_eq!(handlers, 7);

    let actions = nodes
        .iter()
        .map(|node| node.name.as_str())
        .filter(|name| name.starts_with("error-action:"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actions,
        [
            "error-action:propagate",
            "error-action:record",
            "error-action:recover",
        ]
        .into_iter()
        .collect()
    );

    let flows = nodes
        .iter()
        .map(|node| node.name.as_str())
        .filter(|name| name.starts_with("error-flow:"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        flows,
        [
            "error-flow:continuation",
            "error-flow:enclosing-propagation",
            "error-flow:fallback",
        ]
        .into_iter()
        .collect()
    );

    let operations = nodes
        .iter()
        .filter(|node| node.name == "outcome-operation:rust:fallible")
        .collect::<Vec<_>>();
    assert_eq!(operations.len(), 6);

    let action_ids = nodes
        .iter()
        .filter(|node| node.name == "outcome-action:consume")
        .map(|node| node.id.as_str())
        .collect::<BTreeSet<_>>();
    let consumed = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Edge(edge)
                if edge.relation == "contains" && action_ids.contains(edge.target.as_str()) =>
            {
                Some(edge.source.as_str())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(consumed.len(), 4);
}

#[test]
fn rust_discovers_nested_workspace_without_root_manifest() {
    let analysis = analyze("nested_repository");
    for package in ["nested-core", "nested-cli"] {
        assert!(
            analysis.records.iter().any(|record| matches!(
                record,
                FactRecord::Node(node)
                    if node.kind == "module"
                        && node.attributes.as_ref().and_then(|value| value.get("package"))
                            == Some(&serde_json::json!(package))
            )),
            "missing nested package {package}"
        );
    }
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node)
            if node.kind == "file" && node.path.starts_with("native/vector-engine/")
    )));
}

#[test]
fn rust_analysis_is_deterministic() {
    for fixture in ["sample", "semantic_fixture", "nested_repository"] {
        assert_eq!(
            analyze(fixture),
            analyze(fixture),
            "fixture {fixture} changed output"
        );
    }
}

fn node<'a>(records: &'a [FactRecord], qualified: &str) -> &'a lexicon::NodeRecord {
    records
        .iter()
        .find_map(|record| match record {
            FactRecord::Node(node) if node.qualified_name == qualified => Some(node),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing node {qualified}"))
}

fn assert_edge(records: &[FactRecord], source: &str, target: &str, relation: &str) {
    let source = node(records, source);
    let target = node(records, target);
    assert!(
        records.iter().any(|record| matches!(
            record,
            FactRecord::Edge(edge)
                if edge.source == source.id
                    && edge.target == target.id
                    && edge.relation == relation
        )),
        "missing {relation}: {source:?} -> {target:?}"
    );
}

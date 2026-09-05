use crate::orchestrator;
use serde_json::Value;
use std::path::PathBuf;

#[test]
fn emits_normalized_error_handling_capabilities_and_actions() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/semantic_fixture");
    let records: Vec<Value> = orchestrator::generate(&fixture, None, None)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let nodes: Vec<_> = records
        .iter()
        .filter(|record| record["record"] == "node")
        .collect();
    let handlers: Vec<_> = nodes
        .iter()
        .filter(|node| node["name"] == "error-handler:rust")
        .collect();
    let actions: std::collections::BTreeSet<_> = nodes
        .iter()
        .filter_map(|node| node["name"].as_str())
        .filter(|name| name.starts_with("error-action:"))
        .collect();

    assert!(nodes.iter().any(|node| {
        node["name"]
            .as_str()
            .is_some_and(|name| name.starts_with("semantic-capabilities:rust:"))
    }));
    assert_eq!(handlers.len(), 4);
    assert!(!nodes.iter().any(|node| {
        node["path"] == "src/generated.rs"
            && node["qualified_name"]
                .as_str()
                .is_some_and(|name| name.starts_with("@semantic/"))
    }));
    assert_eq!(
        actions,
        [
            "error-action:propagate",
            "error-action:record",
            "error-action:recover"
        ]
        .into_iter()
        .collect()
    );
}

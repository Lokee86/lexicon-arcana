use crate::orchestrator;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[test]
fn emits_consumed_and_unobserved_result_outcomes() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/semantic_fixture");
    let records: Vec<Value> = orchestrator::generate(&fixture, None, None)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let operations: Vec<_> = records
        .iter()
        .filter(|record| {
            record["record"] == "node" && record["name"] == "outcome-operation:rust:fallible"
        })
        .collect();
    let action_ids: BTreeSet<_> = records
        .iter()
        .filter(|record| record["record"] == "node" && record["name"] == "outcome-action:consume")
        .filter_map(|record| record["id"].as_str())
        .collect();
    let consumed_sources: BTreeSet<_> = records
        .iter()
        .filter(|record| {
            record["record"] == "edge"
                && record["relation"] == "contains"
                && record["target"]
                    .as_str()
                    .is_some_and(|target| action_ids.contains(target))
        })
        .filter_map(|record| record["source"].as_str())
        .collect();

    assert_eq!(operations.len(), 6);
    assert_eq!(consumed_sources.len(), 4);
    assert_eq!(
        operations
            .iter()
            .filter(|operation| operation["id"]
                .as_str()
                .is_some_and(|id| !consumed_sources.contains(id)))
            .count(),
        2
    );
}

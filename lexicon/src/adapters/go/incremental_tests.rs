use std::path::PathBuf;

use crate::{AdapterMode, AdapterRequest, FactRecord, LanguageAdapter};

use super::{GoAdapter, tests::real_helper};

#[test]
fn incremental_adapter_emits_complete_scoped_analysis_before_host_filtering() {
    let repository = fixture("basic_calls");
    let adapter = GoAdapter::with_helper(real_helper());
    let full = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository: repository.clone(),
            ..AdapterRequest::default()
        })
        .unwrap();
    let incremental = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            mode: AdapterMode::Incremental,
            repository,
            changed_files: vec!["main.go".into()],
            ..AdapterRequest::default()
        })
        .unwrap();

    assert_eq!(incremental.records, full.records);
    assert_eq!(
        incremental.header.changed_files,
        Some(vec!["main.go".into()])
    );
    assert_eq!(incremental.header.removed_files, Some(Vec::new()));
    assert_eq!(incremental.header.shared_complete, Some(true));
}

#[test]
fn base_incremental_filter_keeps_changed_and_shared_go_facts() {
    let repository = fixture("basic_calls");
    let adapter = GoAdapter::with_helper(real_helper());
    let mut analysis = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            mode: AdapterMode::Incremental,
            repository,
            changed_files: vec!["main.go".into()],
            removed_files: vec!["internal/sub/sub.go".into()],
            ..AdapterRequest::default()
        })
        .unwrap();

    analysis.restrict_incremental_ownership();
    analysis.canonicalize().unwrap();
    analysis.validate().unwrap();

    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node) if node.kind == "repository" && node.owner.is_none()
    )));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node) if node.owner.as_deref() == Some("main.go")
    )));
    assert!(!analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node) if node.owner.as_deref() == Some("internal/sub/sub.go")
    )));
    assert!(!analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node) if node.kind == "file" && node.path == "internal/sub/sub.go"
    )));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == "depends-on"
                && edge.owner.as_deref() == Some("main.go")
                && edge.attributes.as_ref().is_some_and(|attributes|
                    attributes["category"] == "local")
    )));
    assert_eq!(
        analysis.header.removed_files,
        Some(vec!["internal/sub/sub.go".into()])
    );
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("adapters/go/testdata/oracle")
        .join(name)
}

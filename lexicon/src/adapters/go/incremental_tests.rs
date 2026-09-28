use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{AdapterMode, AdapterRequest, FactRecord, LanguageAdapter, build_analysis_scope};

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

#[test]
fn package_scoped_incremental_matches_full_changed_owner() {
    let root = unique_temp("source");
    let temporary = unique_temp("scope");
    write_test_file(&root, "go.mod", "module example.com/scoped\n\ngo 1.22\n");
    write_test_file(
        &root,
        "pkg/a.go",
        "package pkg\n\nimport \"example.com/scoped/dep\"\n\nfunc Changed() { dep.Helper() }\n",
    );
    write_test_file(&root, "pkg/b.go", "package pkg\n\nfunc Sibling() {}\n");
    write_test_file(&root, "dep/dep.go", "package dep\n\nfunc Helper() {}\n");
    write_test_file(
        &root,
        "unrelated/other.go",
        "package unrelated\n\nfunc Other() {}\n",
    );

    let adapter = GoAdapter::with_helper(real_helper());
    let mut expected = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository: root.clone(),
            ..AdapterRequest::default()
        })
        .unwrap();
    expected.header.mode = Some("incremental".into());
    expected.header.changed_files = Some(vec!["pkg/a.go".into()]);
    expected.header.removed_files = Some(Vec::new());
    expected.header.shared_complete = Some(true);
    expected.restrict_incremental_ownership();
    expected.canonicalize().unwrap();

    let scoped = build_analysis_scope(
        &root,
        &temporary,
        "go",
        &["pkg/a.go".into(), "dep/dep.go".into()],
    )
    .unwrap();
    assert!(scoped.join("pkg/a.go").is_file());
    assert!(scoped.join("pkg/b.go").is_file());
    assert!(scoped.join("dep/dep.go").is_file());
    assert!(!scoped.join("unrelated/other.go").exists());

    let mut incremental = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            mode: AdapterMode::Incremental,
            repository: scoped,
            changed_files: vec!["pkg/a.go".into()],
            ..AdapterRequest::default()
        })
        .unwrap();
    incremental.restrict_incremental_ownership();
    incremental.canonicalize().unwrap();
    incremental.validate().unwrap();

    let expected_groups = expected.groups(None);
    let incremental_groups = incremental.groups(None);
    assert_eq!(
        incremental_groups.owned.get("pkg/a.go"),
        expected_groups.owned.get("pkg/a.go")
    );

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(temporary);
}

fn unique_temp(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "lexicon-go-incremental-{label}-{}-{nonce}",
        std::process::id()
    ))
}

fn write_test_file(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/go_oracle/repositories")
        .join(name)
}

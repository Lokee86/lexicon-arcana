mod support;

use lexicon::{
    LanguageEntry, LanguageResult, SnapshotManifest, StorageError, Store, assemble_manifest,
};

use support::TestDirectory;

#[test]
fn manifest_assembly_is_deterministic_and_supports_removal() {
    let manifest = SnapshotManifest {
        version: 1,
        state_commit: "old".into(),
        languages: Some(vec![language("python", "old"), language("ruby", "old")]),
    };
    let assembled = assemble_manifest(
        manifest,
        vec![
            LanguageResult {
                language: "ruby".into(),
                entry: None,
            },
            LanguageResult {
                language: "python".into(),
                entry: Some(language("python", "new")),
            },
            LanguageResult {
                language: "rust".into(),
                entry: Some(language("rust", "new")),
            },
        ],
    )
    .unwrap();

    let languages = assembled.languages.unwrap();
    assert_eq!(
        languages
            .iter()
            .map(|entry| entry.language.as_str())
            .collect::<Vec<_>>(),
        vec!["python", "rust"]
    );
    assert_eq!(languages[0].adapter_version, "new");
}

#[test]
fn manifest_assembly_rejects_mismatched_result_language() {
    let error = assemble_manifest(
        SnapshotManifest {
            version: 1,
            state_commit: "old".into(),
            languages: Some(Vec::new()),
        },
        vec![LanguageResult {
            language: "python".into(),
            entry: Some(language("ruby", "test")),
        }],
    )
    .unwrap_err();
    assert!(matches!(error, StorageError::Materialization(_)));
}

#[test]
fn scan_publication_handoff_matches_go_transaction_order() {
    let directory = TestDirectory::new("scan-transaction");
    let store = Store::new(&directory.path);
    let manifest = SnapshotManifest {
        version: 1,
        state_commit: "base-head".into(),
        languages: Some(vec![language("python", "test")]),
    };

    let transaction = store
        .begin_scan_publication(&manifest, "base-head", true)
        .unwrap();
    let pending = store.pending().unwrap();
    assert_eq!(pending.base_state_commit, "base-head");
    assert!(pending.commit_required);
    assert_eq!(pending.manifest.state_commit, "");
    assert_eq!(transaction.base_state_commit(), "base-head");
    assert!(transaction.commit_required());

    let id = store
        .finish_scan_publication(transaction, "new-head")
        .unwrap();
    let (current_id, current) = store.current().unwrap();
    assert_eq!(current_id, id);
    assert_eq!(current.state_commit, "new-head");
    assert!(matches!(
        store.pending(),
        Err(StorageError::NoPendingPublication)
    ));
}

#[test]
fn failed_finish_retains_pending_for_recovery() {
    let directory = TestDirectory::new("scan-transaction-failure");
    let store = Store::new(&directory.path);
    let manifest = SnapshotManifest {
        version: 1,
        state_commit: "base".into(),
        languages: Some(Vec::new()),
    };
    let transaction = store
        .begin_scan_publication(&manifest, "base", true)
        .unwrap();

    assert!(store.finish_scan_publication(transaction, "").is_err());
    assert!(store.pending().is_ok());
}

fn language(language: &str, adapter_version: &str) -> LanguageEntry {
    LanguageEntry {
        language: language.into(),
        adapter_version: adapter_version.into(),
        adapter_fingerprint: String::new(),
        schema_version: 1,
        repository: "repo".into(),
        analysis_config_id: "config".into(),
        shared_object_id: String::new(),
        dependency_index_id: String::new(),
        files: Some(Vec::new()),
    }
}

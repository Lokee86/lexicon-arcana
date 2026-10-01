mod support;

use std::fs;

use lexicon::{
    CONSUMER_VERSION, ConsumerDefinition, Lexicon, SnapshotManifest, StateRepository, Store,
    save_config, state_root,
};
use serde_json::json;

use support::TestDirectory;

#[test]
fn status_reports_sorted_snapshot_languages_selection_and_consumers() {
    let root = TestDirectory::new("status-report");
    let repository = root.path.join("repository");
    let adapters = root.path.join("adapters");
    fs::create_dir_all(&repository).unwrap();
    fs::create_dir_all(&adapters).unwrap();
    save_config(&repository, &adapters).unwrap();

    let state = state_root(&repository);
    let git = StateRepository::ensure(state.join("repo")).unwrap();
    git.commit_state().unwrap();
    let store = Store::new(&state);
    store
        .publish(&SnapshotManifest {
            version: 1,
            state_commit: git.head().unwrap(),
            languages: Some(vec![language("python"), language("go")]),
        })
        .unwrap();

    let consumers = state.join("consumers");
    fs::create_dir_all(&consumers).unwrap();
    for name in ["zeta.json", "alpha.json", "ignore.txt"] {
        fs::write(consumers.join(name), b"{}").unwrap();
    }

    let lexicon = Lexicon::open(&repository).unwrap();
    let status = lexicon.status().unwrap();
    assert_eq!(status.repository_root, repository);
    assert!(status.current_snapshot_id.is_some());
    assert_eq!(status.detected_languages, vec!["go", "python"]);
    assert!(status.enabled_languages.is_empty());
    assert_eq!(status.registered_consumers, vec!["alpha", "zeta"]);
}

#[test]
fn consumer_definition_preserves_go_timeout_compatibility() {
    let definition = ConsumerDefinition::parse(
        serde_json::to_string(&json!({
            "version": CONSUMER_VERSION,
            "command": "arcana",
            "args": ["sync"],
            "timeout": "1h2m3.5s"
        }))
        .unwrap()
        .as_bytes(),
    )
    .unwrap();
    assert_eq!(definition.command, "arcana");
    assert_eq!(definition.args, vec!["sync"]);
    assert_eq!(definition.timeout_nanos, 3_723_500_000_000);

    let numeric =
        ConsumerDefinition::parse(br#"{"version":1,"command":"arcana","timeout":1234}"#).unwrap();
    assert_eq!(numeric.timeout_nanos, 1234);

    assert!(
        ConsumerDefinition::parse(br#"{"version":1,"command":"arcana","timeout":"-1s"}"#).is_err()
    );
    assert!(ConsumerDefinition::parse(br#"{"version":1}"#).is_err());
}

fn language(name: &str) -> lexicon::LanguageEntry {
    lexicon::LanguageEntry {
        language: name.into(),
        adapter_version: String::new(),
        adapter_fingerprint: String::new(),
        schema_version: 0,
        repository: String::new(),
        analysis_config_id: String::new(),
        shared_object_id: String::new(),
        dependency_index_id: String::new(),
        files: Some(Vec::new()),
    }
}

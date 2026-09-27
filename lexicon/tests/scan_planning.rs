#[path = "support/graph.rs"]
mod graph;
mod support;

use std::collections::BTreeMap;

use lexicon::{
    Change, FactObject, FactRecord, FileEntry, LanguageEntry, PlanningInput, SnapshotManifest,
    Store, plan_scan,
};

use graph::{edge, node, publish_language, write_file};
use support::TestDirectory;

#[test]
fn modified_source_uses_dependency_scoped_incremental_plan() {
    let directory = TestDirectory::new("scan-plan-incremental");
    let store = Store::new(&directory.path);
    publish_language(
        &store,
        vec![
            write_file(&store, "a.py", vec![node("node-a", "a.py")]),
            write_file(&store, "b.py", vec![node("node-b", "b.py")]),
        ],
    );
    let (_, manifest) = store.current().unwrap();

    let plan = plan_scan(
        &store,
        &manifest,
        &PlanningInput {
            changes: vec![change("M", "", "a.py")],
            present_languages: vec!["python".into()],
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(plan.languages(), vec!["python"]);
    let python = &plan.analyses[0];
    assert!(!python.full);
    assert_eq!(python.changed_files, vec!["a.py"]);
    assert_eq!(python.removed_files, Vec::<String>::new());
    assert_eq!(python.context_files, vec!["a.py"]);
}

#[test]
fn go_noop_scan_plans_no_adapter_work() {
    let directory = TestDirectory::new("scan-plan-go-noop");
    let store = Store::new(&directory.path);
    publish_go_language(
        &store,
        vec![write_go_file(
            &store,
            "pkg/impl.go",
            vec![node("impl", "pkg/impl.go")],
        )],
    );
    let (_, manifest) = store.current().unwrap();

    let plan = plan_scan(
        &store,
        &manifest,
        &PlanningInput {
            present_languages: vec!["go".into()],
            enabled_languages: vec!["go".into()],
            ..Default::default()
        },
    )
    .unwrap();

    assert!(plan.analyses.is_empty());
}

#[test]
fn go_implementation_edit_stays_dependency_scoped() {
    let directory = TestDirectory::new("scan-plan-go-implementation");
    let store = Store::new(&directory.path);
    publish_go_language(
        &store,
        vec![
            write_go_file(&store, "pkg/impl.go", vec![node("impl", "pkg/impl.go")]),
            write_go_file(
                &store,
                "unrelated/other.go",
                vec![node("other", "unrelated/other.go")],
            ),
        ],
    );
    let (_, manifest) = store.current().unwrap();

    let plan = plan_scan(
        &store,
        &manifest,
        &PlanningInput {
            changes: vec![change("M", "", "pkg/impl.go")],
            present_languages: vec!["go".into()],
            enabled_languages: vec!["go".into()],
            ..Default::default()
        },
    )
    .unwrap();

    let go = &plan.analyses[0];
    assert!(!go.full);
    assert_eq!(go.changed_files, vec!["pkg/impl.go"]);
    assert_eq!(go.context_files, vec!["pkg/impl.go"]);
}

#[test]
fn go_exported_api_edit_reanalyzes_dependents_without_full_repository() {
    let directory = TestDirectory::new("scan-plan-go-api");
    let store = Store::new(&directory.path);
    publish_go_language(
        &store,
        vec![
            write_go_file(&store, "api/api.go", vec![node("api", "api/api.go")]),
            write_go_file(
                &store,
                "consumer/use.go",
                vec![
                    node("consumer", "consumer/use.go"),
                    edge("consumer", "api", "calls", "consumer/use.go"),
                ],
            ),
            write_go_file(
                &store,
                "unrelated/other.go",
                vec![node("other", "unrelated/other.go")],
            ),
        ],
    );
    let (_, manifest) = store.current().unwrap();

    let plan = plan_scan(
        &store,
        &manifest,
        &PlanningInput {
            changes: vec![change("M", "", "api/api.go")],
            present_languages: vec!["go".into()],
            enabled_languages: vec!["go".into()],
            ..Default::default()
        },
    )
    .unwrap();

    let go = &plan.analyses[0];
    assert!(!go.full);
    assert_eq!(go.changed_files, vec!["api/api.go", "consumer/use.go"]);
    assert_eq!(go.context_files, go.changed_files);
    assert!(!go.context_files.contains(&"unrelated/other.go".into()));
}

#[test]
fn go_added_deleted_and_manifest_changes_force_full_analysis() {
    let directory = TestDirectory::new("scan-plan-go-structural");
    let store = Store::new(&directory.path);
    publish_go_language(
        &store,
        vec![write_go_file(
            &store,
            "pkg/existing.go",
            vec![node("existing", "pkg/existing.go")],
        )],
    );
    let (_, manifest) = store.current().unwrap();

    for change in [
        change("A", "", "pkg/new.go"),
        change("D", "pkg/existing.go", ""),
        change("M", "", "go.mod"),
    ] {
        let plan = plan_scan(
            &store,
            &manifest,
            &PlanningInput {
                changes: vec![change],
                present_languages: vec!["go".into()],
                enabled_languages: vec!["go".into()],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(plan.analyses[0].full);
    }
}

#[test]
fn python_structural_plans_preserve_incremental_add_rename_and_full_drift_boundaries() {
    let directory = TestDirectory::new("scan-plan-full");
    let store = Store::new(&directory.path);
    publish_language(
        &store,
        vec![write_file(&store, "a.py", vec![node("node-a", "a.py")])],
    );
    let (_, mut manifest) = store.current().unwrap();
    manifest.languages.as_mut().unwrap()[0].adapter_fingerprint = "old".into();

    let config = plan_scan(
        &store,
        &manifest,
        &PlanningInput {
            changes: vec![change("M", "", "pyproject.toml")],
            present_languages: vec!["python".into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(config.analyses[0].full);

    let addition = plan_scan(
        &store,
        &manifest,
        &PlanningInput {
            changes: vec![change("A", "", "new.py")],
            present_languages: vec!["python".into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!addition.analyses[0].full);
    assert_eq!(addition.analyses[0].changed_files, vec!["new.py"]);

    let rename = plan_scan(
        &store,
        &manifest,
        &PlanningInput {
            changes: vec![change("R100", "a.py", "renamed.py")],
            present_languages: vec!["python".into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!rename.analyses[0].full);
    assert_eq!(rename.analyses[0].changed_files, vec!["renamed.py"]);
    assert_eq!(rename.analyses[0].removed_files, vec!["a.py"]);

    let drift = plan_scan(
        &store,
        &manifest,
        &PlanningInput {
            present_languages: vec!["python".into()],
            adapter_fingerprints: Some(BTreeMap::from([("python".into(), "new".into())])),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(drift.analyses[0].full);
}

#[test]
fn source_drift_and_disabled_languages_match_go_planning_order() {
    let directory = TestDirectory::new("scan-plan-drift");
    let store = Store::new(&directory.path);
    let python = language("python");
    let ruby = language("ruby");
    let manifest = SnapshotManifest {
        version: 1,
        state_commit: "state".into(),
        languages: Some(vec![python, ruby]),
    };

    let plan = plan_scan(
        &store,
        &manifest,
        &PlanningInput {
            present_languages: vec!["python".into(), "ruby".into(), "rust".into()],
            enabled_languages: vec!["python".into(), "rust".into()],
            ..Default::default()
        },
    )
    .unwrap();

    assert!(plan.pruned_disabled_languages);
    assert!(plan.manifest.language("ruby").is_none());
    assert_eq!(plan.languages(), vec!["rust"]);
    assert!(plan.analyses[0].full);
}

#[test]
fn absent_previous_language_is_planned_for_full_reconciliation() {
    let directory = TestDirectory::new("scan-plan-removed-language");
    let store = Store::new(&directory.path);
    let manifest = SnapshotManifest {
        version: 1,
        state_commit: "state".into(),
        languages: Some(vec![language("python")]),
    };

    let plan = plan_scan(&store, &manifest, &PlanningInput::default()).unwrap();
    assert_eq!(plan.languages(), vec!["python"]);
    assert!(plan.analyses[0].full);
    assert!(!plan.analyses[0].known_present);
}

#[test]
fn generic_source_changes_use_generic_language_identity() {
    let directory = TestDirectory::new("scan-plan-generic");
    let store = Store::new(&directory.path);
    let manifest = SnapshotManifest {
        version: 1,
        state_commit: "state".into(),
        languages: Some(Vec::new()),
    };
    let plan = plan_scan(
        &store,
        &manifest,
        &PlanningInput {
            changes: vec![change("A", "", "scripts/build.ps1")],
            present_languages: vec!["generic-ps1".into()],
            enabled_languages: vec!["generic".into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(plan.languages(), vec!["generic-ps1"]);
    assert!(plan.analyses[0].full);
}

fn write_go_file(store: &Store, path: &str, records: Vec<FactRecord>) -> FileEntry {
    let object = FactObject {
        version: 1,
        language: "go".into(),
        owner: path.into(),
        source_content_id: format!("content:{path}"),
        adapter_version: "test".into(),
        schema_version: 1,
        analysis_config_id: "config".into(),
        records,
    };
    let object_id = store.write_object(&object).unwrap();
    FileEntry {
        path: path.into(),
        language: "go".into(),
        content_id: object.source_content_id,
        object_id,
    }
}

fn publish_go_language(store: &Store, files: Vec<FileEntry>) {
    store
        .publish(&SnapshotManifest {
            version: 1,
            state_commit: "state".into(),
            languages: Some(vec![LanguageEntry {
                language: "go".into(),
                adapter_version: "test".into(),
                adapter_fingerprint: String::new(),
                schema_version: 1,
                repository: "repo".into(),
                analysis_config_id: "config".into(),
                shared_object_id: String::new(),
                files: Some(files),
            }]),
        })
        .unwrap();
}

fn change(status: &str, old: &str, new: &str) -> Change {
    Change {
        status: status.into(),
        old: old.into(),
        new: new.into(),
    }
}

fn language(language: &str) -> LanguageEntry {
    LanguageEntry {
        language: language.into(),
        adapter_version: "test".into(),
        adapter_fingerprint: String::new(),
        schema_version: 1,
        repository: "repo".into(),
        analysis_config_id: "config".into(),
        shared_object_id: String::new(),
        files: Some(Vec::new()),
    }
}

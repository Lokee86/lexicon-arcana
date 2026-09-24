#[path = "support/graph.rs"]
mod graph;
mod support;

use std::collections::BTreeMap;

use lexicon::{Change, LanguageEntry, PlanningInput, SnapshotManifest, Store, plan_scan};

use graph::{node, publish_language, write_file};
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
fn structural_changes_and_adapter_drift_force_full_analysis() {
    let directory = TestDirectory::new("scan-plan-full");
    let store = Store::new(&directory.path);
    publish_language(
        &store,
        vec![write_file(&store, "a.py", vec![node("node-a", "a.py")])],
    );
    let (_, mut manifest) = store.current().unwrap();
    manifest.languages.as_mut().unwrap()[0].adapter_fingerprint = "old".into();

    for change in [
        change("M", "", "pyproject.toml"),
        change("A", "", "new.py"),
        change("R100", "a.py", "renamed.py"),
    ] {
        let plan = plan_scan(
            &store,
            &manifest,
            &PlanningInput {
                changes: vec![change],
                present_languages: vec!["python".into()],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(plan.analyses[0].full);
    }

    let plan = plan_scan(
        &store,
        &manifest,
        &PlanningInput {
            present_languages: vec!["python".into()],
            adapter_fingerprints: Some(BTreeMap::from([("python".into(), "new".into())])),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(plan.analyses[0].full);
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

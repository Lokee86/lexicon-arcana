mod support;

use std::fs;
use std::thread;
use std::time::Duration;

use lexicon::{
    FactObject, FileEntry, GcOptions, GcPlan, LanguageEntry, SnapshotManifest, Store, content_id,
};

use support::TestDirectory;

#[test]
fn gc_preserves_current_retention_and_consumer_pin_and_supports_dry_run() {
    let root = TestDirectory::new("gc-plan");
    let store = Store::new(root.path.join("store"));

    let old_object = write_object(&store, "old.py");
    let pinned_object = write_object(&store, "pinned.py");
    let newest_object = write_object(&store, "newest.py");
    let unreachable = write_object(&store, "unreachable.py");

    let old = publish(&store, "old", &old_object);
    thread::sleep(Duration::from_millis(10));
    let pinned = publish(&store, "pinned", &pinned_object);
    thread::sleep(Duration::from_millis(10));
    let newest = publish(&store, "newest", &newest_object);

    let pin_dir = store.root().join("consumer-state");
    fs::create_dir_all(&pin_dir).unwrap();
    fs::write(
        pin_dir.join("arcana.json"),
        format!(r#"{{"snapshot_id":"{pinned}"}}"#),
    )
    .unwrap();

    fs::write(store.root().join("CURRENT"), format!("{old}\n")).unwrap();

    let plan = store.plan_gc(GcOptions { keep_snapshots: 1 }).unwrap();
    assert_eq!(plan.current_snapshot, old);
    assert_eq!(
        plan.preserved_snapshots,
        sorted(vec![old.clone(), pinned.clone(), newest.clone()])
    );
    assert!(plan.delete_snapshots.is_empty());
    assert_eq!(plan.delete_objects, vec![unreachable.clone()]);

    let dry = store.execute_gc(plan.clone(), true).unwrap();
    assert!(dry.dry_run);
    assert_eq!(dry.deleted_objects, vec![unreachable.clone()]);
    assert!(store.object_path(&unreachable).exists());

    let result = store.execute_gc(plan, false).unwrap();
    assert!(!result.dry_run);
    assert_eq!(result.deleted_objects, vec![unreachable.clone()]);
    assert!(!store.object_path(&unreachable).exists());
    assert!(store.object_path(&old_object).exists());
    assert!(store.object_path(&pinned_object).exists());
    assert!(store.object_path(&newest_object).exists());
}

#[test]
fn gc_deletes_unpreserved_snapshot_and_objects() {
    let root = TestDirectory::new("gc-delete");
    let store = Store::new(root.path.join("store"));
    let old_object = write_object(&store, "old.py");
    let current_object = write_object(&store, "current.py");

    let old = publish(&store, "old", &old_object);
    let current = publish(&store, "current", &current_object);

    let dry = store
        .garbage_collect(GcOptions { keep_snapshots: 0 }, true)
        .unwrap();
    assert_eq!(dry.deleted_snapshots, vec![old.clone()]);
    assert_eq!(dry.deleted_objects, vec![old_object.clone()]);
    assert!(snapshot_path(&store, &old).exists());

    let result = store
        .garbage_collect(GcOptions { keep_snapshots: 0 }, false)
        .unwrap();
    assert_eq!(result.deleted_snapshots, vec![old.clone()]);
    assert!(!snapshot_path(&store, &old).exists());
    assert!(!store.object_path(&old_object).exists());
    assert_eq!(store.current().unwrap().0, current);
}

#[test]
fn gc_rejects_malformed_or_missing_consumer_pins() {
    for (name, data) in [
        ("invalid-json", "{"),
        ("missing-field", "{}"),
        ("invalid-id", r#"{"snapshot_id":"not-an-id"}"#),
    ] {
        let root = TestDirectory::new(name);
        let store = Store::new(root.path.join("store"));
        store
            .publish(&SnapshotManifest {
                version: 1,
                state_commit: "current".into(),
                languages: Some(Vec::new()),
            })
            .unwrap();
        let directory = store.root().join("consumer-state");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("pin.json"), data).unwrap();
        assert!(store.plan_gc(GcOptions::default()).is_err());
    }

    let root = TestDirectory::new("missing-pin");
    let store = Store::new(root.path.join("store"));
    store
        .publish(&SnapshotManifest {
            version: 1,
            state_commit: "current".into(),
            languages: Some(Vec::new()),
        })
        .unwrap();
    let directory = store.root().join("consumer-state");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("pin.json"),
        r#"{"snapshot_id":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"}"#,
    )
    .unwrap();
    assert!(store.plan_gc(GcOptions::default()).is_err());
}

#[test]
fn execute_gc_rejects_changed_current_and_invalid_plan() {
    let root = TestDirectory::new("gc-current-change");
    let store = Store::new(root.path.join("store"));
    let old_object = write_object(&store, "old.py");
    let old = publish(&store, "old", &old_object);
    let planned = store
        .publish(&SnapshotManifest {
            version: 1,
            state_commit: "planned".into(),
            languages: Some(Vec::new()),
        })
        .unwrap();
    let plan = store.plan_gc(GcOptions::default()).unwrap();
    assert_eq!(plan.current_snapshot, planned);

    store
        .publish(&SnapshotManifest {
            version: 1,
            state_commit: "changed".into(),
            languages: Some(Vec::new()),
        })
        .unwrap();
    assert!(store.execute_gc(plan, false).is_err());
    assert!(snapshot_path(&store, &old).exists());

    let invalid = GcPlan {
        current_snapshot: old.clone(),
        preserved_snapshots: vec![old.clone()],
        delete_snapshots: vec![old],
        preserved_objects: Vec::new(),
        delete_objects: Vec::new(),
        preserved_topology_objects: Vec::new(),
        delete_topology_objects: Vec::new(),
        delete_bootstrap_snapshots: Vec::new(),
    };
    assert!(store.execute_gc(invalid, true).is_err());
}

fn write_object(store: &Store, owner: &str) -> String {
    store
        .write_object(&FactObject {
            version: 1,
            language: "python".into(),
            owner: owner.into(),
            source_content_id: content_id(owner.as_bytes()),
            adapter_version: "test".into(),
            schema_version: 1,
            analysis_config_id: "sha256:config".into(),
            records: Vec::new(),
        })
        .unwrap()
}

fn publish(store: &Store, state: &str, object: &str) -> String {
    store
        .publish(&SnapshotManifest {
            version: 1,
            state_commit: state.into(),
            languages: Some(vec![LanguageEntry {
                language: "python".into(),
                adapter_version: String::new(),
                adapter_fingerprint: String::new(),
                schema_version: 0,
                repository: String::new(),
                analysis_config_id: String::new(),
                shared_object_id: String::new(),
                dependency_index_id: String::new(),
                files: Some(vec![FileEntry {
                    path: "file.py".into(),
                    language: "python".into(),
                    content_id: String::new(),
                    object_id: object.into(),
                }]),
            }]),
        })
        .unwrap()
}

fn snapshot_path(store: &Store, id: &str) -> std::path::PathBuf {
    store
        .root()
        .join("snapshots")
        .join(format!("{}.json", id.trim_start_matches("sha256:")))
}

fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

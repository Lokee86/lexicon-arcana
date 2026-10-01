mod support;

use lexicon::{GcOptions, SnapshotManifest, Store};
use std::fs;
use support::TestDirectory;

#[test]
fn gc_retains_pinned_index_shards_and_prunes_orphans_and_legacy_bootstraps() {
    use lexicon::{Analysis, FactHeader, SourceFile};
    let root = TestDirectory::new("gc-index-pins");
    let store = Store::new(root.path.join("store"));
    let analysis = Analysis::new(
        FactHeader {
            adapter_version: "test".into(),
            changed_files: None,
            language: "python".into(),
            mode: Some("full".into()),
            record: "lexicon".into(),
            removed_files: None,
            repository: "repo".into(),
            schema_version: 1,
            shared_complete: None,
        },
        Vec::new(),
    );
    let build = |contents: &str, config: &str| {
        store
            .build_full_language(
                &analysis,
                &[SourceFile {
                    path: "a.py".into(),
                    content: contents.as_bytes().to_vec(),
                }],
                "python",
                config,
                "fp",
            )
            .unwrap()
    };
    let old_entry = build("old", "cfg");
    let old_root = old_entry.dependency_index_id.clone();
    let old = store
        .publish(&SnapshotManifest {
            version: 1,
            state_commit: "old".into(),
            languages: Some(vec![old_entry.clone()]),
        })
        .unwrap();
    let new_entry = build("new", "cfg");
    let new_root = new_entry.dependency_index_id.clone();
    let new = store
        .publish(&SnapshotManifest {
            version: 1,
            state_commit: "new".into(),
            languages: Some(vec![new_entry.clone()]),
        })
        .unwrap();
    let orphan = build("orphan", "different");
    let orphan_root = orphan.dependency_index_id.clone();
    let path = |id: &str| {
        let hex = id.trim_start_matches("sha256:");
        store
            .root()
            .join("topology/objects")
            .join(&hex[..2])
            .join(&hex[2..])
    };
    // Executing a plan after a new consumer pin is added must fail closed.
    let stale = store.plan_gc(GcOptions::default()).unwrap();
    let pin = store.root().join("consumer-state/arcana.json");
    fs::create_dir_all(pin.parent().unwrap()).unwrap();
    fs::write(&pin, format!(r#"{{"snapshot_id":"{old}"}}"#)).unwrap();
    assert!(store.execute_gc(stale, false).is_err());
    assert!(path(&old_root).exists());
    let plan = store.plan_gc(GcOptions::default()).unwrap();
    assert!(plan.preserved_topology_objects.contains(&old_root));
    assert!(plan.preserved_topology_objects.contains(&new_root));
    assert!(plan.delete_topology_objects.contains(&orphan_root));
    store.execute_gc(plan, false).unwrap();
    assert!(path(&old_root).exists() && path(&new_root).exists());
    assert!(!path(&orphan_root).exists());
    fs::remove_file(pin).unwrap();
    let gc = store.garbage_collect(GcOptions::default(), false).unwrap();
    assert!(gc.deleted_snapshots.contains(&old));
    assert!(!path(&old_root).exists());
    assert!(path(&new_root).exists());

    // A one-time legacy bootstrap is retained for its snapshot but removed
    // with that snapshot after CURRENT advances to an indexed generation.
    let mut legacy = new_entry.clone();
    legacy.dependency_index_id.clear();
    let legacy_id = store
        .publish(&SnapshotManifest {
            version: 1,
            state_commit: "legacy".into(),
            languages: Some(vec![legacy]),
        })
        .unwrap();
    store.incremental_scope("python", &["a.py".into()]).unwrap();
    let legacy_dir = store
        .root()
        .join("topology/bootstrap")
        .join(legacy_id.trim_start_matches("sha256:"));
    assert!(legacy_dir.exists());
    store
        .publish(&SnapshotManifest {
            version: 1,
            state_commit: "after-legacy".into(),
            languages: Some(vec![new_entry]),
        })
        .unwrap();
    let plan = store.plan_gc(GcOptions::default()).unwrap();
    assert!(plan.delete_bootstrap_snapshots.contains(&legacy_id));
    store.execute_gc(plan, false).unwrap();
    assert!(!legacy_dir.exists());
    assert!(path(&new_root).exists());
    assert_ne!(new, old);
}

#[path = "support/dependency_delta_fixture.rs"]
#[allow(dead_code)]
mod fixture;
mod support;

use fixture::{State, header, node, publish};
use lexicon::{Analysis, SnapshotManifest, Store};
use support::TestDirectory;

#[test]
fn interrupted_publication_preserves_current_and_recovers_index_atomically() {
    use lexicon::{GcOptions, RecoveryOutcome};
    use std::fs;

    let directory = TestDirectory::new("delta-recover");
    let store = Store::new(&directory.path);
    let mut state = State::initial();
    let first = Analysis::new(header("full", &[], &[]), state.all_records());
    let entry = store
        .build_full_language(&first, &state.sources(), "python", "cfg", "fp")
        .unwrap();
    publish(&store, entry, 0);
    let original = store.current().unwrap().0;
    state.revisions.insert("a.py".into(), 2);
    state.files.insert(
        "a.py".into(),
        vec![node("node-a-new", Some("a.py"), "a.py")],
    );
    let changed = Analysis::new(
        header("incremental", &["a.py"], &[]),
        state.files["a.py"].clone(),
    );
    let (_, previous) = store.current().unwrap();
    let next = store
        .build_incremental_language(
            previous.language("python").unwrap(),
            &changed,
            &state.changed("a.py"),
            "cfg",
            "fp",
            &["a.py".into()],
            &[],
            false,
        )
        .unwrap();
    let hex = next.dependency_index_id.trim_start_matches("sha256:");
    let path = directory
        .path
        .join("topology/objects")
        .join(&hex[..2])
        .join(&hex[2..]);
    let intact = fs::read(&path).unwrap();
    let candidate = SnapshotManifest {
        version: 1,
        state_commit: "unused".into(),
        languages: Some(vec![next]),
    };
    store.write_pending("state-0", true, &candidate).unwrap();
    fs::write(&path, b"corrupt").unwrap();
    assert!(store.recover_pending(Some("state-1")).is_err());
    assert_eq!(store.current().unwrap().0, original);
    assert!(store.pending().is_ok());
    assert!(store.garbage_collect(GcOptions::default(), false).is_err());
    fs::write(path, intact).unwrap();
    let recovered = store.recover_pending(Some("state-1")).unwrap();
    let RecoveryOutcome::Published(snapshot) = recovered else {
        panic!("not recovered")
    };
    assert_eq!(store.current().unwrap().0, snapshot);
    assert!(store.pending().is_err());
    fs::rename(
        directory.path.join("objects"),
        directory.path.join("hidden-facts"),
    )
    .unwrap();
    assert!(
        !store
            .incremental_scope("python", &["a.py".into()])
            .unwrap()
            .full_required
    );
}

#[test]
fn complete_shared_replacement_rejects_incremental_index() {
    let directory = TestDirectory::new("delta-shared-hard-cut");
    let store = Store::new(&directory.path);
    let state = State::initial();
    let full = Analysis::new(header("full", &[], &[]), state.all_records());
    let prior = store
        .build_full_language(&full, &state.sources(), "python", "cfg", "fp")
        .unwrap();
    let scoped = Analysis::new(
        header("incremental", &["a.py"], &[]),
        state.files["a.py"].clone(),
    );
    assert!(
        store
            .build_incremental_language(
                &prior,
                &scoped,
                &state.changed("a.py"),
                "cfg",
                "fp",
                &["a.py".into()],
                &[],
                true,
            )
            .is_err()
    );
}

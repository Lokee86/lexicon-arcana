#[path = "support/dependency_delta_fixture.rs"]
#[allow(dead_code)]
mod fixture;
mod support;

use std::fs;

use fixture::{State, header, node, publish};
use lexicon::{Analysis, Store};
use support::TestDirectory;

#[test]
fn legacy_bootstrap_and_first_delta_match_canonical_full_index() {
    let directory = TestDirectory::new("phase4-migrated-delta");
    let migrated = Store::new(directory.path.join("migrated"));
    let oracle = Store::new(directory.path.join("oracle"));
    let mut state = State::initial();
    let full = Analysis::new(header("full", &[], &[]), state.all_records());
    let mut previous = migrated
        .build_full_language(&full, &state.sources(), "python", "cfg", "fp")
        .unwrap();
    previous.dependency_index_id.clear();
    publish(&migrated, previous, 0);

    let roots = ["c.py".into()];
    let first = migrated.incremental_scope("python", &roots).unwrap();
    let (old_snapshot, entry) = migrated.current().unwrap();
    let marker = directory
        .path
        .join("migrated/topology/bootstrap")
        .join(old_snapshot.trim_start_matches("sha256:"));
    assert_eq!(fs::read_dir(marker).unwrap().count(), 1);

    // Prove subsequent queries and the first incremental index writer never
    // require the old per-file fact objects after one-time migration.
    let objects = directory.path.join("migrated/objects");
    fs::rename(&objects, directory.path.join("old-facts-hidden")).unwrap();
    assert_eq!(migrated.incremental_scope("python", &roots).unwrap(), first);

    state
        .files
        .insert("e.py".into(), vec![node("node-e", Some("e.py"), "e.py")]);
    state.revisions.insert("e.py".into(), 1);
    let incremental = Analysis::new(
        header("incremental", &["e.py"], &[]),
        state.files["e.py"].clone(),
    );
    let next = migrated
        .build_incremental_language(
            entry.language("python").unwrap(),
            &incremental,
            &state.changed("e.py"),
            "cfg",
            "fp",
            &["e.py".into()],
            &[],
            false,
        )
        .unwrap();

    let full = Analysis::new(header("full", &[], &[]), state.all_records());
    let expected = oracle
        .build_full_language(&full, &state.sources(), "python", "cfg", "fp")
        .unwrap();
    assert_eq!(next.dependency_index_id, expected.dependency_index_id);
}

#[path = "support/dependency_delta_fixture.rs"]
#[allow(dead_code)]
mod fixture;
mod support;

use std::fs;

use fixture::{State, header, node, publish};
use lexicon::{Analysis, SnapshotManifest, Store};
use sha2::{Digest, Sha256};
use support::TestDirectory;

#[test]
fn version_one_index_with_shared_nodes_upgrades_to_exact_full_version_two() {
    let directory = TestDirectory::new("delta-upgrade-v1-shared");
    let store = Store::new(directory.path.join("old"));
    let oracle = Store::new(directory.path.join("oracle"));
    let mut state = State::initial();
    let full = Analysis::new(header("full", &[], &[]), state.all_records());
    let mut previous = store
        .build_full_language(&full, &state.sources(), "python", "cfg", "fp")
        .unwrap();

    // Create a valid historical v1 root lacking shared-path partitions.
    // Its old ownership mapping knows shared-a but not shared-e: e.py did
    // not exist at the time this snapshot was published.
    let old_hex = previous.dependency_index_id.trim_start_matches("sha256:");
    let original = store
        .root()
        .join("topology/objects")
        .join(&old_hex[..2])
        .join(&old_hex[2..]);
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(original).unwrap()).unwrap();
    value["version"] = serde_json::json!(1);
    value.as_object_mut().unwrap().remove("shared_paths");
    let bytes = serde_json::to_vec(&value).unwrap();
    let mut digest = Sha256::new();
    digest.update(b"lexicon:dependency-index:v1\0");
    digest.update(&bytes);
    let id = format!("sha256:{:x}", digest.finalize());
    let hex = id.trim_start_matches("sha256:");
    let path = store
        .root()
        .join("topology/objects")
        .join(&hex[..2])
        .join(&hex[2..]);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();

    previous.dependency_index_id = id;
    publish(&store, previous, 0);
    state
        .files
        .insert("e.py".into(), vec![node("node-e", Some("e.py"), "e.py")]);
    state.revisions.insert("e.py".into(), 1);
    let incremental = Analysis::new(
        header("incremental", &["e.py"], &[]),
        state.files["e.py"].clone(),
    );
    let (_, current) = store.current().unwrap();
    let upgraded = store
        .build_incremental_language(
            current.language("python").unwrap(),
            &incremental,
            &state.changed("e.py"),
            "cfg",
            "fp",
            &["e.py".into()],
            &[],
            false,
        )
        .unwrap();
    let upgraded_id = upgraded.dependency_index_id.clone();
    store
        .publish(&SnapshotManifest {
            version: 1,
            state_commit: "upgraded".into(),
            languages: Some(vec![upgraded]),
        })
        .unwrap();

    let full = Analysis::new(header("full", &[], &[]), state.all_records());
    let expected = oracle
        .build_full_language(&full, &state.sources(), "python", "cfg", "fp")
        .unwrap();
    assert_eq!(upgraded_id, expected.dependency_index_id);
    publish(&oracle, expected, 1);
    let roots = ["e.py".into()];
    assert_eq!(
        store.incremental_scope("python", &roots).unwrap(),
        oracle.incremental_scope("python", &roots).unwrap()
    );
    assert!(
        store
            .incremental_scope("python", &roots)
            .unwrap()
            .emit
            .contains(&"c.py".into())
    );
}

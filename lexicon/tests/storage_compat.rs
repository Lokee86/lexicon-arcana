use lexicon::{FactRecord, SnapshotManifest, decode_object, snapshot_bytes, snapshot_id};

const SNAPSHOT: &str = include_str!("../evaluation/rust_migration/fixtures/snapshot-v1.json");

#[test]
fn snapshot_json_matches_go_reference_bytes() {
    let manifest: SnapshotManifest = serde_json::from_str(SNAPSHOT).expect("snapshot fixture");
    let encoded = snapshot_bytes(&manifest).expect("snapshot bytes");
    assert_eq!(encoded, SNAPSHOT.trim().as_bytes());
    assert_eq!(
        snapshot_id(&manifest).expect("snapshot id"),
        "sha256:af897c1e218e4ce1b7dfb888d1be4669ec1b2395bc3e1914f13a63399d88279e"
    );
}

#[test]
fn snapshot_hash_forces_v1_like_go_publish() {
    let mut manifest: SnapshotManifest = serde_json::from_str(SNAPSHOT).unwrap();
    let expected = snapshot_id(&manifest).unwrap();
    manifest.version = 99;
    assert_eq!(
        snapshot_bytes(&manifest).unwrap(),
        SNAPSHOT.trim().as_bytes()
    );
    assert_eq!(snapshot_id(&manifest).unwrap(), expected);
}

#[test]
fn snapshot_preserves_go_nil_vs_empty_slice_bytes() {
    let nil_languages = SnapshotManifest {
        version: 0,
        state_commit: "abc".into(),
        languages: None,
    };
    assert_eq!(
        snapshot_bytes(&nil_languages).unwrap(),
        br#"{"version":1,"state_commit":"abc","languages":null}"#
    );

    let empty_languages = SnapshotManifest {
        version: 0,
        state_commit: "abc".into(),
        languages: Some(Vec::new()),
    };
    assert_eq!(
        snapshot_bytes(&empty_languages).unwrap(),
        br#"{"version":1,"state_commit":"abc","languages":[]}"#
    );
    assert_ne!(
        snapshot_id(&nil_languages).unwrap(),
        snapshot_id(&empty_languages).unwrap()
    );
}

#[test]
fn legacy_json_object_remains_readable() {
    let object = br#"
    {
      "version": 1,
      "language": "python",
      "owner": "main.py",
      "source_content_id": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "adapter_version": "legacy",
      "schema_version": 1,
      "analysis_config_id": "config",
      "records": [
        {
          "id": "node-1",
          "kind": "function",
          "name": "run",
          "owner": "main.py",
          "path": "main.py",
          "qualified_name": "demo.run",
          "record": "node"
        }
      ]
    }
    "#;
    let decoded = decode_object(object).expect("legacy JSON object");
    assert_eq!(decoded.language, "python");
    assert_eq!(decoded.owner, "main.py");
    assert_eq!(decoded.records.len(), 1);
    assert!(matches!(&decoded.records[0], FactRecord::Node(node) if node.id == "node-1"));
}

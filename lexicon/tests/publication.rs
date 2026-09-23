mod support;

use lexicon::{
    FactObject, RecoveryOutcome, StorageError, Store, encode_object, object_id, snapshot_bytes,
};
use std::fs;

use support::{TestDirectory, manifest};

#[test]
fn current_reports_missing_snapshot() {
    let directory = TestDirectory::new("missing-current");
    let store = Store::new(&directory.path);
    assert!(matches!(
        store.current(),
        Err(StorageError::NoCurrentSnapshot)
    ));
}

#[test]
fn publish_uses_existing_layout_and_current_pointer() {
    let directory = TestDirectory::new("publish");
    let store = Store::new(&directory.path);
    let manifest = manifest("head-1");

    let id = store.publish(&manifest).expect("publish snapshot");
    assert_eq!(
        id,
        "sha256:90576a3b682d81491af51f9a2c8e06a50819b656985881ae64f36156b6e750cb"
    );
    assert_eq!(
        fs::read(directory.path.join("CURRENT")).unwrap(),
        format!("{id}\n").as_bytes()
    );

    let mut expected = snapshot_bytes(&manifest).unwrap();
    expected.push(b'\n');
    assert_eq!(fs::read(store.snapshot_path(&id)).unwrap(), expected);

    let (current_id, current) = store.current().expect("read current");
    assert_eq!(current_id, id);
    assert_eq!(current.state_commit, "head-1");
    assert_eq!(current.version, 1);
}

#[test]
fn object_write_uses_existing_content_addressed_layout() {
    let directory = TestDirectory::new("object-layout");
    let store = Store::new(&directory.path);
    let object = sample_object();

    let encoded = encode_object(&object).unwrap();
    let expected_id = object_id(&encoded);
    let id = store.write_object(&object).expect("write object");
    assert_eq!(id, expected_id);

    let hex = &id[7..];
    assert_eq!(
        store.object_path(&id),
        directory
            .path
            .join("objects")
            .join(&hex[..2])
            .join(&hex[2..])
    );
    assert_eq!(fs::read(store.object_path(&id)).unwrap(), encoded);

    let mut expected = object;
    expected.version = 1;
    assert_eq!(store.load_object(&id).unwrap(), expected);
}

#[test]
fn content_verification_rejects_modified_snapshot() {
    let directory = TestDirectory::new("snapshot-verification");
    let store = Store::new(&directory.path);
    let id = store.publish(&manifest("head")).unwrap();
    fs::write(store.snapshot_path(&id), b"{}\n").unwrap();
    assert!(matches!(
        store.load_snapshot(&id),
        Err(StorageError::Verification(actual)) if actual == id
    ));
}

#[test]
fn immutable_object_collision_is_rejected() {
    let directory = TestDirectory::new("object-collision");
    let store = Store::new(&directory.path);
    let object = sample_object();
    let encoded = encode_object(&object).unwrap();
    let id = object_id(&encoded);
    let path = store.object_path(&id);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"different").unwrap();

    assert!(matches!(
        store.write_object(&object),
        Err(StorageError::Collision(actual)) if actual == path.display().to_string()
    ));
}

#[test]
fn store_lock_serializes_writers() {
    let directory = TestDirectory::new("lock");
    let store = Store::new(&directory.path);
    let first = store.lock().expect("first lock");
    assert!(matches!(store.lock(), Err(StorageError::Busy)));
    drop(first);
    store.lock().expect("lock after release");
}

#[test]
fn no_pending_recovery_is_a_noop() {
    let directory = TestDirectory::new("no-pending");
    let store = Store::new(&directory.path);
    assert_eq!(
        store.recover_pending(Some("head")).unwrap(),
        RecoveryOutcome::NoPending
    );
}

fn sample_object() -> FactObject {
    FactObject {
        version: 99,
        language: "go".into(),
        owner: "main.go".into(),
        source_content_id: "content".into(),
        adapter_version: "1.0.0".into(),
        schema_version: 1,
        analysis_config_id: "config".into(),
        records: Vec::new(),
    }
}

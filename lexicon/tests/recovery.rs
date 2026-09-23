mod support;

use lexicon::{RecoveryOutcome, StorageError, Store};
use std::fs;

use support::{TestDirectory, manifest};

#[test]
fn pending_bytes_match_go_oracle() {
    let directory = TestDirectory::new("pending-bytes");
    let store = Store::new(&directory.path);
    let candidate = manifest("candidate");

    store
        .write_pending("base-head", true, &candidate)
        .expect("write pending");
    assert_eq!(
        fs::read(directory.path.join("PENDING")).unwrap(),
        b"{\"version\":1,\"base_state_commit\":\"base-head\",\"commit_required\":true,\"manifest\":{\"version\":99,\"state_commit\":\"\",\"languages\":[]}}\n"
    );

    let pending = store.pending().expect("read pending");
    assert_eq!(pending.base_state_commit, "base-head");
    assert!(pending.commit_required);
    assert_eq!(pending.manifest.state_commit, "");
    assert_eq!(pending.manifest.version, 99);
}

#[test]
fn recovery_discards_uncommitted_candidate() {
    let directory = TestDirectory::new("recover-discard");
    let store = Store::new(&directory.path);
    let original = store.publish(&manifest("base-head")).unwrap();

    store
        .write_pending("base-head", true, &manifest("candidate"))
        .unwrap();
    assert_eq!(
        store.recover_pending(Some("base-head")).unwrap(),
        RecoveryOutcome::Discarded
    );
    assert!(matches!(
        store.pending(),
        Err(StorageError::NoPendingPublication)
    ));
    assert_eq!(store.current().unwrap().0, original);
    assert_eq!(store.current().unwrap().1.state_commit, "base-head");
}

#[test]
fn recovery_discards_required_commit_when_no_head_exists() {
    let directory = TestDirectory::new("recover-no-head");
    let store = Store::new(&directory.path);
    store
        .write_pending("", true, &manifest("candidate"))
        .unwrap();
    assert_eq!(
        store.recover_pending(None).unwrap(),
        RecoveryOutcome::Discarded
    );
    assert!(matches!(
        store.current(),
        Err(StorageError::NoCurrentSnapshot)
    ));
}

#[test]
fn recovery_attaches_advanced_head_and_publishes() {
    let directory = TestDirectory::new("recover-advanced");
    let store = Store::new(&directory.path);
    store.publish(&manifest("base-head")).unwrap();
    store
        .write_pending("base-head", true, &manifest("candidate"))
        .unwrap();

    let outcome = store.recover_pending(Some("new-head")).unwrap();
    let RecoveryOutcome::Published(id) = outcome else {
        panic!("expected recovered publication");
    };
    let (current_id, current) = store.current().unwrap();
    assert_eq!(current_id, id);
    assert_eq!(current.state_commit, "new-head");
    assert!(matches!(
        store.pending(),
        Err(StorageError::NoPendingPublication)
    ));
}

#[test]
fn recovery_publishes_when_state_commit_was_not_required() {
    let directory = TestDirectory::new("recover-direct");
    let store = Store::new(&directory.path);
    store.publish(&manifest("base-head")).unwrap();
    store
        .write_pending("base-head", false, &manifest("candidate"))
        .unwrap();

    let outcome = store.recover_pending(Some("base-head")).unwrap();
    assert!(matches!(outcome, RecoveryOutcome::Published(_)));
    assert_eq!(store.current().unwrap().1.state_commit, "base-head");
}

#[test]
fn clear_pending_is_idempotent() {
    let directory = TestDirectory::new("clear-pending");
    let store = Store::new(&directory.path);
    store.clear_pending().unwrap();
    store
        .write_pending("base", false, &manifest("candidate"))
        .unwrap();
    store.clear_pending().unwrap();
    store.clear_pending().unwrap();
    assert!(matches!(
        store.pending(),
        Err(StorageError::NoPendingPublication)
    ));
}

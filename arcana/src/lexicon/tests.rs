use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{LexiconSnapshot, LexiconSnapshotMetadata};

#[test]
fn reads_multi_language_snapshot_and_deduplicates_shared_nodes() {
    let directory = TestDirectory::new();
    let root = directory
        .path
        .join(".warlock")
        .join("tools")
        .join("lexicon");
    fs::create_dir_all(root.join("objects")).unwrap();
    fs::create_dir_all(root.join("snapshots")).unwrap();

    let node_id = sha_id("node");
    let node = json!({
        "record": "node",
        "id": node_id,
        "kind": "repository",
        "path": ".lexicon-repository",
        "name": "repo",
        "qualified_name": "repo",
        "owner": null,
        "content_id": null
    });
    let go_object = write_object(&root, "go", vec![node.clone()]);
    let rust_object = write_object(&root, "rust", vec![node]);
    let manifest = json!({
        "version": 1,
        "state_commit": "state",
        "languages": [
            language("go", &go_object),
            language("rust", &rust_object)
        ]
    });
    let snapshot_id = write_snapshot(&root, &manifest);
    fs::write(root.join("CURRENT"), format!("{snapshot_id}\n")).unwrap();

    let snapshot = LexiconSnapshot::current(&root).unwrap();
    assert_eq!(snapshot.id(), snapshot_id);
    assert_eq!(snapshot.facts().nodes.len(), 1);
}

#[test]
fn rejects_modified_content_addressed_manifest() {
    let directory = TestDirectory::new();
    let root = directory.path.join(".lexicon");
    fs::create_dir_all(root.join("snapshots")).unwrap();
    let manifest = json!({"version": 1, "state_commit": "state", "languages": []});
    let snapshot_id = write_snapshot(&root, &manifest);
    let hex = snapshot_id.strip_prefix("sha256:").unwrap();
    fs::write(
        root.join("snapshots").join(format!("{hex}.json")),
        b"{\"version\":1,\"state_commit\":\"changed\",\"languages\":[]}",
    )
    .unwrap();
    assert!(LexiconSnapshot::load(&root, &snapshot_id).is_err());
}

#[test]
fn metadata_load_does_not_read_referenced_fact_objects() {
    let directory = TestDirectory::new();
    let root = directory.path.join(".lexicon");
    fs::create_dir_all(root.join("snapshots")).unwrap();

    let missing_object = sha_id("missing-object");
    let manifest = json!({
        "version": 1,
        "state_commit": "state",
        "languages": [{
            "language": "go",
            "adapter_version": "1",
            "adapter_fingerprint": sha_id("adapter"),
            "schema_version": 1,
            "repository": "repo",
            "analysis_config_id": sha_id("config"),
            "shared_object_id": missing_object,
            "files": []
        }]
    });
    let snapshot_id = write_snapshot(&root, &manifest);
    fs::write(root.join("CURRENT"), format!("{snapshot_id}\n")).unwrap();

    let metadata = LexiconSnapshotMetadata::current(&root).unwrap();
    assert_eq!(metadata.id(), snapshot_id);
    assert!(LexiconSnapshot::current(&root).is_err());
}

#[test]
fn detects_language_shared_object_changes_from_metadata() {
    let mut previous = synthetic_metadata(BTreeMap::new());
    let mut current = synthetic_metadata(BTreeMap::new());
    previous
        .shared_objects
        .insert("go".to_owned(), Some(sha_id("shared-1")));
    current
        .shared_objects
        .insert("go".to_owned(), Some(sha_id("shared-2")));
    assert!(current.shared_objects_changed(&previous));
}

#[test]
fn reports_added_changed_and_removed_file_objects_from_metadata() {
    let previous = synthetic_metadata(BTreeMap::from([
        (("go".to_owned(), "a.go".to_owned()), sha_id("a1")),
        (("go".to_owned(), "gone.go".to_owned()), sha_id("gone")),
    ]));
    let current = synthetic_metadata(BTreeMap::from([
        (("go".to_owned(), "a.go".to_owned()), sha_id("a2")),
        (("go".to_owned(), "new.go".to_owned()), sha_id("new")),
    ]));
    let changes = current.changed_paths(&previous);
    assert_eq!(changes.added, vec!["new.go"]);
    assert_eq!(changes.changed, vec!["a.go"]);
    assert_eq!(changes.removed, vec!["gone.go"]);
}

fn synthetic_metadata(files: BTreeMap<(String, String), String>) -> LexiconSnapshotMetadata {
    LexiconSnapshotMetadata {
        id: sha_id("snapshot"),
        files,
        shared_objects: Default::default(),
    }
}

fn language(language: &str, object_id: &str) -> Value {
    json!({
        "language": language,
        "adapter_version": "1",
        "adapter_fingerprint": sha_id("adapter"),
        "schema_version": 1,
        "repository": "repo",
        "analysis_config_id": sha_id("config"),
        "shared_object_id": object_id,
        "files": []
    })
}

fn write_object(root: &Path, language: &str, records: Vec<Value>) -> String {
    let object = json!({
        "version": 1,
        "language": language,
        "owner": null,
        "source_content_id": null,
        "adapter_version": "1",
        "schema_version": 1,
        "analysis_config_id": sha_id("config"),
        "records": records
    });
    let bytes = serde_json::to_vec(&object).unwrap();
    let id = domain_id("lexicon:fact-object:v1\0", &bytes);
    let hex = id.strip_prefix("sha256:").unwrap();
    let directory = root.join("objects").join(&hex[..2]);
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join(&hex[2..]), bytes).unwrap();
    id
}

fn write_snapshot(root: &Path, manifest: &Value) -> String {
    let bytes = serde_json::to_vec(manifest).unwrap();
    let id = domain_id("lexicon:snapshot:v1\0", &bytes);
    let hex = id.strip_prefix("sha256:").unwrap();
    fs::write(root.join("snapshots").join(format!("{hex}.json")), bytes).unwrap();
    id
}

fn domain_id(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn sha_id(value: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(value.as_bytes()))
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "arcana-lexicon-test-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

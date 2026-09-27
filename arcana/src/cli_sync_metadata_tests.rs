use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use arcana::repository::RepositorySnapshot;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::cli::SyncCommand;
use super::cli_sync::run_sync;

#[test]
fn existing_sync_does_not_read_current_lexicon_objects() {
    let directory = TestDirectory::new();
    let lexicon = directory.path.join(".lexicon");
    let state = directory.path.join(".arcana");
    prepare_lexicon(&lexicon);

    let object_id = write_object(
        &lexicon,
        vec![repository_node("repository", "example/repository")],
    );
    let snapshot_id = write_shared_snapshot(&lexicon, "state", &object_id);
    fs::write(lexicon.join("CURRENT"), format!("{snapshot_id}\n")).unwrap();

    run_sync(&SyncCommand {
        lexicon: lexicon.clone(),
        state: state.clone(),
        register: false,
    })
    .unwrap();

    fs::remove_file(object_path(&lexicon, &object_id)).unwrap();

    let summary = run_sync(&SyncCommand {
        lexicon,
        state,
        register: false,
    })
    .unwrap();
    assert!(summary.contains("mode=existing"));
}

#[test]
fn shared_object_change_bypasses_previous_full_state() {
    let directory = TestDirectory::new();
    let lexicon = directory.path.join(".lexicon");
    let state = directory.path.join(".arcana");
    prepare_lexicon(&lexicon);

    let previous_object = write_object(
        &lexicon,
        vec![repository_node("previous", "example/previous")],
    );
    let previous_id = write_shared_snapshot(&lexicon, "previous", &previous_object);
    fs::write(lexicon.join("CURRENT"), format!("{previous_id}\n")).unwrap();

    run_sync(&SyncCommand {
        lexicon: lexicon.clone(),
        state: state.clone(),
        register: false,
    })
    .unwrap();

    let previous_output = state
        .join("snapshots")
        .join(previous_id.strip_prefix("sha256:").unwrap());
    fs::remove_file(object_path(&lexicon, &previous_object)).unwrap();
    fs::remove_file(previous_output.join("repository.arcana")).unwrap();

    let current_object = write_object(
        &lexicon,
        vec![repository_node("current", "example/current")],
    );
    let current_id = write_shared_snapshot(&lexicon, "current", &current_object);
    fs::write(lexicon.join("CURRENT"), format!("{current_id}\n")).unwrap();

    let summary = run_sync(&SyncCommand {
        lexicon,
        state: state.clone(),
        register: false,
    })
    .unwrap();
    assert!(summary.contains("mode=rebuild"));

    let current_output = state
        .join("snapshots")
        .join(current_id.strip_prefix("sha256:").unwrap());
    let snapshot = RepositorySnapshot::open(current_output.join("repository.manifest")).unwrap();
    assert_eq!(snapshot.facts().nodes.len(), 1);
}

fn prepare_lexicon(root: &Path) {
    fs::create_dir_all(root.join("objects")).unwrap();
    fs::create_dir_all(root.join("snapshots")).unwrap();
}

fn repository_node(seed: &str, name: &str) -> Value {
    json!({
        "record": "node",
        "id": sha_id(seed),
        "kind": "repository",
        "path": ".lexicon-repository",
        "name": name,
        "qualified_name": name
    })
}

fn write_object(root: &Path, records: Vec<Value>) -> String {
    let object = json!({
        "version": 1,
        "language": "go",
        "owner": null,
        "source_content_id": null,
        "adapter_version": "1",
        "schema_version": 1,
        "analysis_config_id": sha_id("config"),
        "records": records
    });
    let bytes = serde_json::to_vec(&object).unwrap();
    let id = domain_id("lexicon:fact-object:v1\0", &bytes);
    let path = object_path(root, &id);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
    id
}

fn write_shared_snapshot(root: &Path, state_commit: &str, object_id: &str) -> String {
    let manifest = json!({
        "version": 1,
        "state_commit": state_commit,
        "languages": [{
            "language": "go",
            "adapter_version": "1",
            "schema_version": 1,
            "repository": "example/repository",
            "analysis_config_id": sha_id("config"),
            "shared_object_id": object_id,
            "files": []
        }]
    });
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let id = domain_id("lexicon:snapshot:v1\0", &bytes);
    let digest = id.strip_prefix("sha256:").unwrap();
    fs::write(root.join("snapshots").join(format!("{digest}.json")), bytes).unwrap();
    id
}

fn object_path(root: &Path, id: &str) -> PathBuf {
    let digest = id.strip_prefix("sha256:").unwrap();
    root.join("objects").join(&digest[..2]).join(&digest[2..])
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
            "arcana-sync-metadata-test-{}-{}",
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

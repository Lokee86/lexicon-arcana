use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;
use sha2::{Digest, Sha256};

use super::snapshot::load as load_rich;
use super::snapshot_compact::{load_and_write_compact, load_compact};
use crate::repository::{compile_compact_repository_graph, compile_repository_graph};
use crate::repository_store::{CompactRepositoryBuild, write_repository_store};

const GOLDEN_V2_HEX: &str = "4c584f424a00020001010900000005312e302e30000964656d6f2e6d61696e010d796e616d69632d746172676574000166010228290002676f00046d61696e04032e676f060801bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb0101cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc007a020001bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb0111111111111111111111111111111111111111111111111111111111111111110308010101000000000122222222222222222222222222222222222222222222222222222222222222220b0701010002010802010202090100010100020000010b0100040005010304000200";

#[test]
fn v2_snapshot_streams_directly_to_the_phase3_compact_oracle() {
    let temp = TestDirectory::new();
    let root = temp.path.join(".lexicon");
    fs::create_dir_all(root.join("objects")).unwrap();
    fs::create_dir_all(root.join("snapshots")).unwrap();

    let object = decode_hex(GOLDEN_V2_HEX);
    let object_id = write_object(&root, &object);
    let config = format!("sha256:{}", "cc".repeat(32));
    let content = format!("sha256:{}", "bb".repeat(32));
    let manifest = json!({
        "version": 1,
        "state_commit": "state",
        "languages": [{
            "language": "go",
            "adapter_version": "1.0.0",
            "adapter_fingerprint": null,
            "schema_version": 1,
            "repository": "repo",
            "analysis_config_id": config,
            "shared_object_id": null,
            "files": [{
                "path": "main.go",
                "language": "go",
                "content_id": content,
                "object_id": object_id
            }]
        }]
    });
    let snapshot_id = write_snapshot(&root, &manifest);

    let rich = load_rich(&root, &snapshot_id).unwrap();
    let expected = CompactRepositoryBuild::from_facts(rich.facts()).unwrap();
    let compact = load_compact(&root, &snapshot_id).unwrap();

    assert!(compact.direct_v2);
    assert_eq!(compact.metadata.id(), rich.id());
    assert_eq!(
        compact.compatibility_warnings,
        rich.compatibility_warnings()
    );
    assert_eq!(compact.repository, expected);

    let rich_store = temp.path.join("rich.arcana");
    let compact_store = temp.path.join("compact.arcana");
    write_repository_store(&rich_store, rich.facts()).unwrap();
    let (_, compact_write) = load_and_write_compact(&root, &snapshot_id, &compact_store).unwrap();
    assert_eq!(
        fs::read(&rich_store).unwrap(),
        fs::read(&compact_store).unwrap()
    );
    assert_eq!(
        compact_write.header.file_len,
        fs::metadata(&compact_store).unwrap().len()
    );

    let rich_graph = compile_repository_graph(rich.facts()).unwrap();
    let compact_graph = compile_compact_repository_graph(&compact.repository).unwrap();
    assert_eq!(compact_graph, rich_graph);
}

#[test]
fn legacy_snapshot_uses_the_compatibility_loader() {
    let temp = TestDirectory::new();
    let root = temp.path.join(".lexicon");
    fs::create_dir_all(root.join("objects")).unwrap();
    fs::create_dir_all(root.join("snapshots")).unwrap();

    let config = sha_id("config");
    let node_id = sha_id("node");
    let object = serde_json::to_vec(&json!({
        "version": 1,
        "language": "go",
        "owner": null,
        "source_content_id": null,
        "adapter_version": "1",
        "schema_version": 1,
        "analysis_config_id": config,
        "records": [{
            "record": "node",
            "id": node_id,
            "kind": "repository",
            "path": "repo",
            "name": "repo",
            "qualified_name": "repo",
            "owner": null,
            "content_id": null
        }]
    }))
    .unwrap();
    let object_id = write_object(&root, &object);
    let manifest = json!({
        "version": 1,
        "state_commit": "state",
        "languages": [{
            "language": "go",
            "adapter_version": "1",
            "adapter_fingerprint": null,
            "schema_version": 1,
            "repository": "repo",
            "analysis_config_id": config,
            "shared_object_id": object_id,
            "files": []
        }]
    });
    let snapshot_id = write_snapshot(&root, &manifest);

    let rich = load_rich(&root, &snapshot_id).unwrap();
    let expected = CompactRepositoryBuild::from_facts(rich.facts()).unwrap();
    let compact = load_compact(&root, &snapshot_id).unwrap();

    assert!(!compact.direct_v2);
    assert_eq!(compact.repository, expected);
}

fn write_object(root: &Path, bytes: &[u8]) -> String {
    let id = domain_id("lexicon:fact-object:v1\0", bytes);
    let hex = id.strip_prefix("sha256:").unwrap();
    let directory = root.join("objects").join(&hex[..2]);
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join(&hex[2..]), bytes).unwrap();
    id
}

fn write_snapshot(root: &Path, value: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap();
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

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| (hex(pair[0]) << 4) | hex(pair[1]))
        .collect()
}

fn hex(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("invalid hex"),
    }
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "arcana-compact-lexicon-test-{}-{}",
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

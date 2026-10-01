use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;
use sha2::{Digest, Sha256};

use super::metadata::LexiconSnapshotMetadata;
use super::snapshot::load as load_rich;
use super::snapshot_compact::{load_and_write_compact, load_compact, load_compact_delta};
use crate::repository::{
    IncrementalError, NodeFact, NodeKey, NodeKind, RepositoryFacts,
    compile_compact_repository_graph, compile_repository_graph,
    verify_compact_delta_node_set_from_store,
};
use crate::repository_store::{
    CompactRepositoryBuild, RepositoryStoreFile, write_repository_store,
    write_repository_store_compact,
};

const GOLDEN_V2_HEX: &str = "4c584f424a00020001010900000005312e302e30000964656d6f2e6d61696e010d796e616d69632d746172676574000166010228290002676f00046d61696e04032e676f060801bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb0101cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc007a020001bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb0111111111111111111111111111111111111111111111111111111111111111110308010101000000000122222222222222222222222222222222222222222222222222222222222222220b0701010002010802010202090100010100020000010b0100040005010304000200";

#[test]
fn v2_snapshot_streams_directly_to_the_compact_oracle() {
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
fn legacy_snapshot_ingests_object_locally_into_compact_build() {
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

#[test]
fn compact_delta_reads_only_selected_objects_and_resolves_unchanged_targets_from_base() {
    let temp = TestDirectory::new();
    let root = temp.path.join(".lexicon");
    fs::create_dir_all(root.join("objects")).unwrap();
    fs::create_dir_all(root.join("snapshots")).unwrap();

    let config = sha_id("delta-config");
    let source = sha_id("delta-source");
    let target = sha_id("delta-target");
    let old_content = sha_id("delta-old-content");
    let new_content = sha_id("delta-new-content");
    let target_content = sha_id("delta-target-content");

    let old_source_object = write_json_object(
        &root,
        "go",
        Some("src/a.go"),
        Some(&old_content),
        &config,
        json!([
            node_record(&source, "src/a.go", "source"),
            edge_record(&source, &target)
        ]),
    );
    let new_source_object = write_json_object(
        &root,
        "go",
        Some("src/a.go"),
        Some(&new_content),
        &config,
        json!([
            node_record(&source, "src/a.go", "source"),
            edge_record(&source, &target)
        ]),
    );
    let target_object = write_json_object(
        &root,
        "go",
        Some("src/b.go"),
        Some(&target_content),
        &config,
        json!([node_record(&target, "src/b.go", "target")]),
    );

    let previous_manifest = two_file_manifest(
        "previous",
        &config,
        &old_content,
        &old_source_object,
        &target_content,
        &target_object,
    );
    let previous_id = write_snapshot(&root, &previous_manifest);
    let previous = load_compact(&root, &previous_id).unwrap();
    let base_path = temp.path.join("base.arcana");
    write_repository_store_compact(&base_path, &previous.repository).unwrap();

    let current_manifest = two_file_manifest(
        "current",
        &config,
        &new_content,
        &new_source_object,
        &target_content,
        &target_object,
    );
    let current_id = write_snapshot(&root, &current_manifest);
    let current = LexiconSnapshotMetadata::load(&root, &current_id).unwrap();

    remove_object(&root, &target_object);

    let changed_paths = vec!["src/a.go".to_owned()];
    let mut base = RepositoryStoreFile::open(&base_path).unwrap();
    let delta = load_compact_delta(&root, &current, &changed_paths, &mut base).unwrap();

    assert_eq!(delta.repository.node_count(), 1);
    assert_eq!(delta.repository.edge_count(), 1);
    assert_eq!(delta.repository.unresolved_count(), 0);
    assert_eq!(
        delta.owned_node_keys(),
        vec![NodeKey::from_identity(source.as_bytes())]
    );
    verify_compact_delta_node_set_from_store(&mut base, &delta.repository, &changed_paths).unwrap();
}

#[test]
fn compact_delta_rejects_base_node_with_same_key_but_wrong_external_identity() {
    let temp = TestDirectory::new();
    let root = temp.path.join(".lexicon");
    fs::create_dir_all(root.join("objects")).unwrap();
    fs::create_dir_all(root.join("snapshots")).unwrap();

    let config = sha_id("identity-config");
    let source = sha_id("identity-source");
    let target = sha_id("identity-target");
    let wrong_target = sha_id("wrong-identity");
    let source_content = sha_id("identity-source-content");
    let target_content = sha_id("identity-target-content");

    let source_object = write_json_object(
        &root,
        "go",
        Some("src/a.go"),
        Some(&source_content),
        &config,
        json!([
            node_record(&source, "src/a.go", "source"),
            edge_record(&source, &target)
        ]),
    );
    let target_object = write_json_object(
        &root,
        "go",
        Some("src/b.go"),
        Some(&target_content),
        &config,
        json!([node_record(&target, "src/b.go", "target")]),
    );
    let manifest = two_file_manifest(
        "identity-current",
        &config,
        &source_content,
        &source_object,
        &target_content,
        &target_object,
    );
    let id = write_snapshot(&root, &manifest);
    let current = LexiconSnapshotMetadata::load(&root, &id).unwrap();

    let base_path = temp.path.join("wrong-base.arcana");
    write_repository_store(
        &base_path,
        &RepositoryFacts::new(
            vec![NodeFact {
                key: NodeKey::from_identity(target.as_bytes()),
                external_identity: Some(wrong_target),
                kind: NodeKind::Function,
                path: "src/b.go".to_owned(),
                name: "target".to_owned(),
                qualified_name: "target".to_owned(),
                content_id: None,
                span: None,
            }],
            vec![],
        ),
    )
    .unwrap();

    remove_object(&root, &target_object);

    let mut base = RepositoryStoreFile::open(&base_path).unwrap();
    let error =
        load_compact_delta(&root, &current, &["src/a.go".to_owned()], &mut base).unwrap_err();
    assert!(matches!(
        error,
        super::LexiconSnapshotError::Malformed("unknown relationship node")
    ));
}

#[test]
fn compact_delta_node_set_verification_detects_changed_node_identity() {
    let temp = TestDirectory::new();
    let root = temp.path.join(".lexicon");
    fs::create_dir_all(root.join("objects")).unwrap();
    fs::create_dir_all(root.join("snapshots")).unwrap();

    let config = sha_id("nodeset-config");
    let previous_node = sha_id("nodeset-previous");
    let current_node = sha_id("nodeset-current");
    let old_content = sha_id("nodeset-old-content");
    let new_content = sha_id("nodeset-new-content");

    let old_object = write_json_object(
        &root,
        "go",
        Some("src/a.go"),
        Some(&old_content),
        &config,
        json!([node_record(&previous_node, "src/a.go", "source")]),
    );
    let old_manifest = one_file_manifest("nodeset-old", &config, &old_content, &old_object);
    let old_id = write_snapshot(&root, &old_manifest);
    let previous = load_compact(&root, &old_id).unwrap();
    let base_path = temp.path.join("nodeset-base.arcana");
    write_repository_store_compact(&base_path, &previous.repository).unwrap();

    let new_object = write_json_object(
        &root,
        "go",
        Some("src/a.go"),
        Some(&new_content),
        &config,
        json!([node_record(&current_node, "src/a.go", "source")]),
    );
    let new_manifest = one_file_manifest("nodeset-new", &config, &new_content, &new_object);
    let new_id = write_snapshot(&root, &new_manifest);
    let current = LexiconSnapshotMetadata::load(&root, &new_id).unwrap();

    let changed_paths = vec!["src/a.go".to_owned()];
    let mut base = RepositoryStoreFile::open(&base_path).unwrap();
    let delta = load_compact_delta(&root, &current, &changed_paths, &mut base).unwrap();
    let error =
        verify_compact_delta_node_set_from_store(&mut base, &delta.repository, &changed_paths)
            .unwrap_err();

    assert!(matches!(error, IncrementalError::NodeSetChanged { .. }));
}

#[test]
fn compact_delta_removed_path_requires_no_current_object() {
    let temp = TestDirectory::new();
    let root = temp.path.join(".lexicon");
    fs::create_dir_all(root.join("objects")).unwrap();
    fs::create_dir_all(root.join("snapshots")).unwrap();

    let config = sha_id("removed-config");
    let remaining = sha_id("remaining-node");
    let remaining_content = sha_id("remaining-content");
    let remaining_object = write_json_object(
        &root,
        "go",
        Some("src/b.go"),
        Some(&remaining_content),
        &config,
        json!([node_record(&remaining, "src/b.go", "remaining")]),
    );
    let manifest = json!({
        "version": 1,
        "state_commit": "removed-current",
        "languages": [{
            "language": "go",
            "adapter_version": "1",
            "adapter_fingerprint": null,
            "schema_version": 1,
            "repository": "repo",
            "analysis_config_id": config,
            "shared_object_id": null,
            "files": [{
                "path": "src/b.go",
                "language": "go",
                "content_id": remaining_content,
                "object_id": remaining_object
            }]
        }]
    });
    let id = write_snapshot(&root, &manifest);
    let current = LexiconSnapshotMetadata::load(&root, &id).unwrap();

    remove_object(&root, &remaining_object);

    let base_path = temp.path.join("removed-base.arcana");
    write_repository_store(&base_path, &RepositoryFacts::default()).unwrap();
    let mut base = RepositoryStoreFile::open(&base_path).unwrap();
    let delta =
        load_compact_delta(&root, &current, &["src/deleted.go".to_owned()], &mut base).unwrap();

    assert_eq!(delta.repository.node_count(), 0);
    assert_eq!(delta.repository.edge_count(), 0);
    assert_eq!(delta.repository.unresolved_count(), 0);
}

fn write_json_object(
    root: &Path,
    language: &str,
    owner: Option<&str>,
    content_id: Option<&str>,
    config: &str,
    records: serde_json::Value,
) -> String {
    let bytes = serde_json::to_vec(&json!({
        "version": 1,
        "language": language,
        "owner": owner,
        "source_content_id": content_id,
        "adapter_version": "1",
        "schema_version": 1,
        "analysis_config_id": config,
        "records": records,
    }))
    .unwrap();
    write_object(root, &bytes)
}

fn node_record(id: &str, path: &str, name: &str) -> serde_json::Value {
    json!({
        "record": "node",
        "id": id,
        "kind": "function",
        "path": path,
        "name": name,
        "qualified_name": name,
        "owner": null,
        "content_id": null
    })
}

fn edge_record(source: &str, target: &str) -> serde_json::Value {
    json!({
        "record": "edge",
        "relation": "calls",
        "source": source,
        "target": target,
        "owner": null
    })
}

fn one_file_manifest(state: &str, config: &str, content: &str, object: &str) -> serde_json::Value {
    json!({
        "version": 1,
        "state_commit": state,
        "languages": [{
            "language": "go",
            "adapter_version": "1",
            "adapter_fingerprint": null,
            "schema_version": 1,
            "repository": "repo",
            "analysis_config_id": config,
            "shared_object_id": null,
            "files": [{
                "path": "src/a.go",
                "language": "go",
                "content_id": content,
                "object_id": object
            }]
        }]
    })
}

fn two_file_manifest(
    state: &str,
    config: &str,
    source_content: &str,
    source_object: &str,
    target_content: &str,
    target_object: &str,
) -> serde_json::Value {
    json!({
        "version": 1,
        "state_commit": state,
        "languages": [{
            "language": "go",
            "adapter_version": "1",
            "adapter_fingerprint": null,
            "schema_version": 1,
            "repository": "repo",
            "analysis_config_id": config,
            "shared_object_id": null,
            "files": [
                {
                    "path": "src/a.go",
                    "language": "go",
                    "content_id": source_content,
                    "object_id": source_object
                },
                {
                    "path": "src/b.go",
                    "language": "go",
                    "content_id": target_content,
                    "object_id": target_object
                }
            ]
        }]
    })
}

fn remove_object(root: &Path, id: &str) {
    let hex = id.strip_prefix("sha256:").unwrap();
    fs::remove_file(root.join("objects").join(&hex[..2]).join(&hex[2..])).unwrap();
}

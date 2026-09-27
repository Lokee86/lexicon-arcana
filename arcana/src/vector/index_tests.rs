use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::repository::{
    EdgeFact, NodeFact, NodeKey, NodeKind, PublishRepositorySnapshot, RelationKind,
    RepositoryFacts, RepositorySnapshot, compile_repository_facts, publish_repository_snapshot,
};
use crate::repository_store::write_repository_store;
use crate::snapshot::publish_snapshot;
use crate::storage::write_packed;

use super::build::replace_index;
use super::{
    BuildOptions, Embedder, EmbeddingError, IndexManifest, SEMANTIC_ELIGIBILITY_POLICY_VERSION,
    build_current_index, build_current_index_with_options, current_index_directory,
    search_current_index, search_expected_index,
};

#[test]
fn builds_reuses_and_searches_current_graph_index() {
    let directory = TestDirectory::new();
    let state = directory.path.join(".arcana");
    let digest = "a".repeat(64);
    let snapshot_directory = state.join("snapshots").join(&digest);
    fs::create_dir_all(&snapshot_directory).unwrap();

    let create = NodeKey::from_u64(1);
    let insert = NodeKey::from_u64(2);
    let facts = RepositoryFacts {
        nodes: vec![
            node(create, "create_profile", "src/profile.rs"),
            node(insert, "insert_profile", "src/repository.rs"),
            node_with_kind(
                NodeKey::from_u64(3),
                NodeKind::Variable,
                "temporary_profile",
                "src/profile.rs",
            ),
        ],
        edges: vec![EdgeFact {
            source: create,
            target: insert,
            relation: RelationKind::Calls,
            span: None,
        }],
        unresolved: Vec::new(),
    };
    publish_test_snapshot(&snapshot_directory, &facts);
    let graph = RepositorySnapshot::open(snapshot_directory.join("repository.manifest")).unwrap();
    assert_eq!(graph.facts().nodes.len(), 3);
    assert_eq!(graph.graph().node_count(), 3);
    fs::write(state.join("CURRENT"), format!("sha256:{digest}\n")).unwrap();

    let embedder = FakeEmbedder;
    let built = build_current_index(&state, &embedder, 1).unwrap();
    assert_eq!(built.mode, "built");
    assert_eq!(built.item_count, 2);
    assert_eq!(built.embedded_vectors, 2);
    assert_eq!(built.reused_vectors, 0);
    assert_eq!(built.request_count, 2);
    assert!(!built.cached_snapshot);
    assert!(built.snapshot_bytes > 32);
    assert_eq!(built.dimensions, 4);
    assert_eq!(
        built.directory,
        current_index_directory(&state, "fake-4d").unwrap()
    );

    let reused = build_current_index(&state, &embedder, 2).unwrap();
    assert_eq!(reused.mode, "existing");
    assert!(reused.cached_snapshot);
    assert_eq!(reused.embedded_vectors, 0);
    assert_eq!(reused.reused_vectors, 2);

    let manifest_path = built.directory.join("manifest.json");
    let manifest: IndexManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    assert_eq!(manifest.version, 3);
    assert_eq!(
        manifest.eligibility_policy_version,
        SEMANTIC_ELIGIBILITY_POLICY_VERSION
    );
    assert_eq!(manifest.identity, "fake-4d-arcana-semantic-v6");

    let mut legacy: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    legacy["version"] = 2.into();
    legacy["identity"] = "fake-4d".into();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("eligibility_policy_version");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&legacy).unwrap()).unwrap();
    let policy_rebuilt = build_current_index(&state, &embedder, 2).unwrap();
    assert_eq!(policy_rebuilt.mode, "built");

    fs::write(built.directory.join("nodes.jsonl"), b"").unwrap();
    let repaired = build_current_index(&state, &embedder, 2).unwrap();
    assert_eq!(repaired.mode, "built");

    let vectors_path = built.directory.join("vectors.f32");
    let mut vectors = fs::read(&vectors_path).unwrap();
    vectors[3] = 0x3e;
    fs::write(&vectors_path, vectors).unwrap();
    let checksum_repaired = build_current_index(&state, &embedder, 2).unwrap();
    assert_eq!(checksum_repaired.mode, "built");

    let hits = search_current_index(&state, &embedder, "create profile", 1).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].name, "create_profile");
    assert_eq!(hits[0].score, 1.0);

    let error = search_expected_index(
        &state,
        &format!("sha256:{}", "b".repeat(64)),
        &embedder,
        "create profile",
        1,
    )
    .unwrap_err();
    assert!(error.to_string().contains("expected snapshot"));

    let mut manifest: IndexManifest =
        serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    manifest.repository_snapshot_id = "0000000000000000".to_owned();
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let error = search_current_index(&state, &embedder, "create profile", 1).unwrap_err();
    assert!(error.to_string().contains("stale"));
}

#[test]
fn exact_snapshot_reuse_makes_no_embedding_requests() {
    let directory = TestDirectory::new();
    let state = directory.path.join(".arcana");
    let digest = "b".repeat(64);
    publish_state_snapshot(
        &state,
        &digest,
        &isolated_facts(&[(1, "alpha"), (2, "beta"), (3, "gamma")]),
    );
    let embedder = TrackingEmbedder::new(0, Duration::ZERO);

    let built = build_current_index_with_options(
        &state,
        &embedder,
        BuildOptions {
            batch_size: 2,
            batch_concurrency: 2,
        },
    )
    .unwrap();
    assert_eq!(built.embedded_vectors, 3);
    assert_eq!(built.request_count, 2);
    assert_eq!(embedder.calls.load(Ordering::SeqCst), 2);

    let reused = build_current_index_with_options(
        &state,
        &embedder,
        BuildOptions {
            batch_size: 1,
            batch_concurrency: 4,
        },
    )
    .unwrap();
    assert!(reused.cached_snapshot);
    assert_eq!(reused.reused_vectors, 3);
    assert_eq!(reused.request_count, 0);
    assert_eq!(embedder.calls.load(Ordering::SeqCst), 2);
}

#[test]
fn cross_snapshot_reuses_only_identical_rendered_documents() {
    let directory = TestDirectory::new();
    let state = directory.path.join(".arcana");
    let first_digest = "c".repeat(64);
    publish_state_snapshot(
        &state,
        &first_digest,
        &isolated_facts(&[(1, "stable"), (2, "renamed")]),
    );
    let embedder = TrackingEmbedder::new(0, Duration::ZERO);
    build_current_index(&state, &embedder, 8).unwrap();

    let second_digest = "d".repeat(64);
    // A new node key does not affect rendered content, while a changed name does.
    publish_state_snapshot(
        &state,
        &second_digest,
        &isolated_facts(&[(9, "stable"), (2, "changed")]),
    );
    let rebuilt = build_current_index(&state, &embedder, 8).unwrap();
    assert!(!rebuilt.cached_snapshot);
    assert_eq!(rebuilt.embedded_vectors, 1);
    assert_eq!(rebuilt.reused_vectors, 1);
    assert_eq!(rebuilt.request_count, 1);
}

#[test]
fn interrupted_build_resumes_from_persisted_batch_objects() {
    let directory = TestDirectory::new();
    let state = directory.path.join(".arcana");
    let digest = "e".repeat(64);
    publish_state_snapshot(
        &state,
        &digest,
        &isolated_facts(&[(1, "one"), (2, "two"), (3, "three")]),
    );
    let embedder = TrackingEmbedder::new(2, Duration::ZERO);
    let options = BuildOptions {
        batch_size: 1,
        batch_concurrency: 1,
    };

    let error = build_current_index_with_options(&state, &embedder, options).unwrap_err();
    assert!(error.to_string().contains("intentional interruption"));
    assert_eq!(embedder.calls.load(Ordering::SeqCst), 2);

    let resumed = build_current_index_with_options(&state, &embedder, options).unwrap();
    assert_eq!(resumed.embedded_vectors, 2);
    assert_eq!(resumed.reused_vectors, 1);
    assert_eq!(resumed.request_count, 2);
    assert_eq!(embedder.calls.load(Ordering::SeqCst), 4);
}

#[test]
fn embedding_batch_concurrency_is_bounded() {
    let directory = TestDirectory::new();
    let state = directory.path.join(".arcana");
    let digest = "f".repeat(64);
    publish_state_snapshot(
        &state,
        &digest,
        &isolated_facts(&[
            (1, "one"),
            (2, "two"),
            (3, "three"),
            (4, "four"),
            (5, "five"),
            (6, "six"),
        ]),
    );
    let embedder = TrackingEmbedder::new(0, Duration::from_millis(30));

    let built = build_current_index_with_options(
        &state,
        &embedder,
        BuildOptions {
            batch_size: 1,
            batch_concurrency: 2,
        },
    )
    .unwrap();
    assert_eq!(built.request_count, 6);
    assert_eq!(embedder.max_active.load(Ordering::SeqCst), 2);
}

#[test]
fn oversized_embedding_batches_split_until_the_provider_accepts_them() {
    let directory = TestDirectory::new();
    let state = directory.path.join(".arcana");
    let digest = "7".repeat(64);
    publish_state_snapshot(
        &state,
        &digest,
        &isolated_facts(&[
            (1, "one"),
            (2, "two"),
            (3, "three"),
            (4, "four"),
            (5, "five"),
        ]),
    );
    let embedder = BatchLimitedEmbedder {
        calls: AtomicUsize::new(0),
        max_batch: 2,
    };

    let built = build_current_index_with_options(
        &state,
        &embedder,
        BuildOptions {
            batch_size: 4,
            batch_concurrency: 2,
        },
    )
    .unwrap();

    assert_eq!(built.embedded_vectors, 5);
    assert_eq!(built.request_count, 4);
    assert_eq!(embedder.calls.load(Ordering::SeqCst), 4);
}

#[test]
fn corrupted_cache_object_is_reembedded_and_repaired() {
    let directory = TestDirectory::new();
    let state = directory.path.join(".arcana");
    let digest = "1".repeat(64);
    publish_state_snapshot(&state, &digest, &isolated_facts(&[(1, "repair")]));
    let embedder = TrackingEmbedder::new(0, Duration::ZERO);
    let built = build_current_index(&state, &embedder, 8).unwrap();
    assert_eq!(embedder.calls.load(Ordering::SeqCst), 1);

    fs::remove_dir_all(&built.directory).unwrap();
    let object = first_cache_object(&state.join("vector-cache")).unwrap();
    let mut bytes = fs::read(&object).unwrap();
    *bytes.last_mut().unwrap() ^= 0xff;
    fs::write(&object, bytes).unwrap();

    let repaired = build_current_index(&state, &embedder, 8).unwrap();
    assert_eq!(repaired.embedded_vectors, 1);
    assert_eq!(repaired.reused_vectors, 0);
    assert_eq!(embedder.calls.load(Ordering::SeqCst), 2);
    assert!(
        build_current_index(&state, &embedder, 8)
            .unwrap()
            .cached_snapshot
    );
}

#[test]
fn publication_failure_restores_the_previous_index() {
    let directory = TestDirectory::new();
    let target = directory.path.join("published");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("marker"), "prior").unwrap();
    let missing_temp = directory.path.join("missing-temp");

    assert!(replace_index(&missing_temp, &target).is_err());
    assert_eq!(fs::read_to_string(target.join("marker")).unwrap(), "prior");
}

fn publish_test_snapshot(directory: &Path, facts: &RepositoryFacts) {
    let compiled = compile_repository_facts(facts).unwrap();
    write_packed(directory.join("graph.arcana"), &compiled.dataset).unwrap();
    publish_snapshot(directory.join("graph.manifest"), "graph.arcana", None, 1).unwrap();
    write_repository_store(directory.join("repository.arcana"), facts).unwrap();
    publish_repository_snapshot(
        directory.join("repository.manifest"),
        PublishRepositorySnapshot {
            graph_manifest_file: Path::new("graph.manifest"),
            repository_store_file: Path::new("repository.arcana"),
            adapter_name: "test",
            adapter_version: "1",
            created_unix_seconds: 1,
        },
    )
    .unwrap();
}

fn publish_state_snapshot(state: &Path, digest: &str, facts: &RepositoryFacts) {
    let snapshot_directory = state.join("snapshots").join(digest);
    fs::create_dir_all(&snapshot_directory).unwrap();
    publish_test_snapshot(&snapshot_directory, facts);
    fs::write(state.join("CURRENT"), format!("sha256:{digest}\n")).unwrap();
}

fn isolated_facts(nodes: &[(u64, &str)]) -> RepositoryFacts {
    RepositoryFacts {
        nodes: nodes
            .iter()
            .map(|(key, name)| node(NodeKey::from_u64(*key), name, &format!("src/{name}.rs")))
            .collect(),
        edges: Vec::new(),
        unresolved: Vec::new(),
    }
}

fn first_cache_object(root: &Path) -> Option<PathBuf> {
    for identity in fs::read_dir(root).ok()? {
        let objects = identity.ok()?.path().join("objects");
        let Ok(prefixes) = fs::read_dir(objects) else {
            continue;
        };
        for prefix in prefixes.flatten() {
            for object in fs::read_dir(prefix.path()).ok()?.flatten() {
                if object
                    .path()
                    .extension()
                    .is_some_and(|value| value == "avec")
                {
                    return Some(object.path());
                }
            }
        }
    }
    None
}

fn node(key: NodeKey, name: &str, path: &str) -> NodeFact {
    NodeFact {
        key,
        external_identity: None,
        kind: NodeKind::Function,
        path: path.to_owned(),
        name: name.to_owned(),
        qualified_name: name.to_owned(),
        content_id: None,
        span: None,
    }
}

fn node_with_kind(key: NodeKey, kind: NodeKind, name: &str, path: &str) -> NodeFact {
    NodeFact {
        key,
        external_identity: None,
        kind,
        path: path.to_owned(),
        name: name.to_owned(),
        qualified_name: name.to_owned(),
        content_id: None,
        span: None,
    }
}

struct FakeEmbedder;

impl Embedder for FakeEmbedder {
    fn model(&self) -> &str {
        "fake-model"
    }

    fn identity(&self) -> &str {
        "fake-4d"
    }

    fn dimensions(&self) -> usize {
        4
    }

    fn embed_documents(&self, documents: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(documents
            .iter()
            .map(|document| {
                if document.contains("name: create_profile\n") {
                    vec![1.0, 0.0, 0.0, 0.0]
                } else {
                    vec![0.0, 1.0, 0.0, 0.0]
                }
            })
            .collect())
    }

    fn embed_query(&self, _query: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vec![1.0, 0.0, 0.0, 0.0])
    }
}

struct BatchLimitedEmbedder {
    calls: AtomicUsize,
    max_batch: usize,
}

impl Embedder for BatchLimitedEmbedder {
    fn model(&self) -> &str {
        "batch-limited-model"
    }

    fn identity(&self) -> &str {
        "batch-limited-4d"
    }

    fn dimensions(&self) -> usize {
        4
    }

    fn embed_documents(&self, documents: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if documents.len() > self.max_batch {
            return Err(EmbeddingError::Service(
                "request exceeds the available context size".to_owned(),
            ));
        }
        Ok(documents.iter().map(|_| vec![1.0, 0.0, 0.0, 0.0]).collect())
    }

    fn embed_query(&self, _query: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vec![1.0, 0.0, 0.0, 0.0])
    }
}

struct TrackingEmbedder {
    calls: AtomicUsize,
    active: AtomicUsize,
    max_active: AtomicUsize,
    fail_on_call: usize,
    delay: Duration,
}

impl TrackingEmbedder {
    fn new(fail_on_call: usize, delay: Duration) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
            fail_on_call,
            delay,
        }
    }
}

impl Embedder for TrackingEmbedder {
    fn model(&self) -> &str {
        "tracking-model"
    }

    fn identity(&self) -> &str {
        "tracking-4d"
    }

    fn dimensions(&self) -> usize {
        4
    }

    fn embed_documents(&self, documents: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_active.fetch_max(active, Ordering::SeqCst);
        std::thread::sleep(self.delay);
        self.active.fetch_sub(1, Ordering::SeqCst);
        if call == self.fail_on_call {
            return Err(EmbeddingError::Service(
                "intentional interruption".to_owned(),
            ));
        }
        Ok(documents.iter().map(|_| vec![1.0, 0.0, 0.0, 0.0]).collect())
    }

    fn embed_query(&self, _query: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vec![1.0, 0.0, 0.0, 0.0])
    }
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "arcana-vector-index-test-{}-{}",
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

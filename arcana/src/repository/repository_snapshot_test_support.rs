use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::repository_store::write_repository_store;
use crate::snapshot::publish_snapshot;
use crate::storage::write_packed;

use super::{
    CompiledRepository, EdgeFact, NodeFact, NodeKey, NodeKind, PublishRepositorySnapshot,
    RelationKind, RepositoryArtifactChecksums, RepositoryFacts, compile_repository_facts,
    repository_artifact_file_checksum,
};

pub fn sample_facts() -> RepositoryFacts {
    RepositoryFacts {
        nodes: vec![
            node(1, NodeKind::Repository, "repo", "repo"),
            node(2, NodeKind::Function, "a.go", "a"),
            node(3, NodeKind::Function, "b.go", "b"),
        ],
        edges: vec![EdgeFact {
            source: NodeKey::from_u64(2),
            target: NodeKey::from_u64(3),
            relation: RelationKind::Calls,
            span: None,
        }],
        unresolved: vec![],
    }
}

pub fn request() -> PublishRepositorySnapshot<'static> {
    PublishRepositorySnapshot {
        graph_manifest_file: Path::new("graph.manifest"),
        repository_store_file: Path::new("repository.arcana"),
        adapter_name: "test",
        adapter_version: "1",
        created_unix_seconds: 7,
    }
}

pub fn write_artifacts(
    directory: &Path,
    facts: &RepositoryFacts,
) -> (CompiledRepository, RepositoryArtifactChecksums) {
    let compiled = compile_repository_facts(facts).unwrap();
    write_packed(directory.join("graph.arcana"), &compiled.dataset).unwrap();
    publish_snapshot(directory.join("graph.manifest"), "graph.arcana", None, 7).unwrap();
    write_repository_store(directory.join("repository.arcana"), facts).unwrap();
    let repository_store =
        repository_artifact_file_checksum(directory.join("repository.arcana")).unwrap();
    (compiled, RepositoryArtifactChecksums { repository_store })
}

pub fn test_directory() -> PathBuf {
    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "arcana-repository-snapshot-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir(&path).unwrap();
    path
}

fn node(key: u64, kind: NodeKind, path: &str, name: &str) -> NodeFact {
    NodeFact {
        key: NodeKey::from_u64(key),
        external_identity: None,
        kind,
        path: path.to_owned(),
        name: name.to_owned(),
        qualified_name: name.to_owned(),
        content_id: None,
        span: None,
    }
}

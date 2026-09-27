use std::fs;

use crate::repository_store::write_repository_store;
use crate::snapshot::publish_snapshot;
use crate::storage::write_packed;

use super::repository_snapshot_test_support::{request, sample_facts, test_directory};
use super::*;

#[test]
fn graph_only_publication_avoids_full_metadata_compilation() {
    let directory = test_directory();
    let facts = sample_facts();
    let graph = compile_repository_graph(&facts).unwrap();

    write_packed(directory.join("graph.arcana"), &graph.dataset).unwrap();
    publish_snapshot(directory.join("graph.manifest"), "graph.arcana", None, 7).unwrap();
    let store_write = write_repository_store(directory.join("repository.arcana"), &facts).unwrap();

    let checksum = repository_artifact_file_checksum(directory.join("repository.arcana")).unwrap();
    let repository_id = repository_identity_for_facts(&facts, checksum);

    compiler::reset_compile_invocation_count();
    let manifest = publish_graph_repository_snapshot_with_identity(
        directory.join(REPOSITORY_MANIFEST_FILE),
        request(),
        &graph,
        repository_id,
        RepositoryArtifactChecksums {
            repository_store: checksum,
        },
        store_write,
    )
    .unwrap();

    assert_eq!(compiler::compile_invocation_count(), 0);
    assert_eq!(manifest.node_count, graph.dataset.node_count);
    assert_eq!(manifest.edge_count, graph.dataset.edges.len() as u64);
    assert_eq!(manifest.unresolved_count, facts.unresolved.len() as u64);

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn graph_only_publication_rejects_store_node_identity_mismatch() {
    let directory = test_directory();
    let facts = sample_facts();
    let mut other = sample_facts();
    other.nodes[1].key = NodeKey::from_u64(99);
    other.edges[0].source = NodeKey::from_u64(99);
    let graph = compile_repository_graph(&facts).unwrap();

    write_packed(directory.join("graph.arcana"), &graph.dataset).unwrap();
    publish_snapshot(directory.join("graph.manifest"), "graph.arcana", None, 7).unwrap();
    let store_write = write_repository_store(directory.join("repository.arcana"), &other).unwrap();

    let checksum = repository_artifact_file_checksum(directory.join("repository.arcana")).unwrap();
    assert!(matches!(
        publish_graph_repository_snapshot_with_identity(
            directory.join(REPOSITORY_MANIFEST_FILE),
            request(),
            &graph,
            repository_identity_for_facts(&other, checksum),
            RepositoryArtifactChecksums {
                repository_store: checksum,
            },
            store_write,
        ),
        Err(RepositorySnapshotError::ArtifactMismatch {
            field: "repository_store_node_keys",
            ..
        })
    ));

    fs::remove_dir_all(directory).unwrap();
}

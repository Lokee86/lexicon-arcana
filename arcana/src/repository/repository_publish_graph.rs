use std::fs;
use std::path::Path;

use crate::repository_store::RepositoryStoreWrite;
use crate::repository_store::format::{FORMAT_VERSION, SectionKind};
use crate::snapshot::GraphSnapshot;
use crate::storage::{StableHasher, dataset_checksum};

use super::repository_snapshot_validation::{checksum, compare, write_immutable};
use super::{
    CompiledRepositoryGraph, PublishRepositorySnapshot, RepositoryArtifactChecksums,
    RepositorySnapshotError, RepositorySnapshotManifest, derive_repository_snapshot_id,
    repository_artifact_file_checksum,
};

pub fn publish_graph_repository_snapshot_with_identity(
    manifest_path: impl AsRef<Path>,
    request: PublishRepositorySnapshot<'_>,
    compiled: &CompiledRepositoryGraph,
    repository_id: u64,
    checksums: RepositoryArtifactChecksums,
    store_write: RepositoryStoreWrite,
) -> Result<RepositorySnapshotManifest, RepositorySnapshotError> {
    let manifest_path = manifest_path.as_ref();
    let root = manifest_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let store_path = root.join(request.repository_store_file);
    compare(
        "repository_store_checksum",
        checksums.repository_store,
        repository_artifact_file_checksum(&store_path)?,
    )?;
    validate_store_graph(compiled, store_write)?;

    let graph = GraphSnapshot::open(root.join(request.graph_manifest_file))?;
    compare(
        "node_count",
        u64::from(compiled.dataset.node_count),
        u64::from(graph.node_count()),
    )?;
    compare(
        "compiled_checksum",
        dataset_checksum(compiled.dataset.node_count, &compiled.dataset.edges),
        graph.dataset_checksum(),
    )?;

    let graph_manifest_checksum = checksum(&fs::read(root.join(request.graph_manifest_file))?);
    let snapshot_id = derive_repository_snapshot_id(
        repository_id,
        graph.snapshot_id(),
        checksums.repository_store,
        request.adapter_name,
        request.adapter_version,
    );
    let manifest = RepositorySnapshotManifest {
        snapshot_id,
        created_unix_seconds: request.created_unix_seconds,
        repository_id,
        adapter_name: request.adapter_name.to_owned(),
        adapter_version: request.adapter_version.to_owned(),
        repository_store_version: FORMAT_VERSION,
        node_count: graph.node_count(),
        edge_count: graph.edge_count(),
        unresolved_count: store_write
            .header
            .section(SectionKind::Unresolved)
            .record_count,
        graph_snapshot_id: graph.snapshot_id(),
        graph_manifest_checksum,
        repository_store_checksum: checksums.repository_store,
        graph_manifest_file: request.graph_manifest_file.to_path_buf(),
        repository_store_file: request.repository_store_file.to_path_buf(),
    };
    write_immutable(manifest_path, manifest.encode()?.as_bytes())?;
    Ok(manifest)
}

fn validate_store_graph(
    compiled: &CompiledRepositoryGraph,
    store_write: RepositoryStoreWrite,
) -> Result<(), RepositorySnapshotError> {
    compare(
        "repository_store_node_count",
        u64::from(compiled.dataset.node_count),
        store_write.header.section(SectionKind::Nodes).record_count,
    )?;
    compare(
        "repository_store_node_keys",
        compiled_node_key_checksum(compiled),
        store_write.node_key_checksum,
    )
}

fn compiled_node_key_checksum(compiled: &CompiledRepositoryGraph) -> u64 {
    let mut hasher = StableHasher::new();
    for key in &compiled.node_keys {
        hasher.update(&key.0.to_le_bytes());
    }
    hasher.finish()
}

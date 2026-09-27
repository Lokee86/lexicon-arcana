use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::Path;

use crate::repository_store::RepositoryStore;
use crate::repository_store::format::FORMAT_VERSION;
use crate::snapshot::GraphSnapshot;
use crate::storage::StableHasher;

use super::repository_snapshot_validation::{
    checksum, compare, repository_identity_from_checksum, validate_compiled_components,
    write_immutable,
};
use super::{
    CompiledRepository, RepositoryFacts, RepositorySnapshotError, RepositorySnapshotManifest,
    compile_repository_facts,
};

pub struct PublishRepositorySnapshot<'a> {
    pub graph_manifest_file: &'a Path,
    pub repository_store_file: &'a Path,
    pub adapter_name: &'a str,
    pub adapter_version: &'a str,
    pub created_unix_seconds: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepositoryArtifactChecksums {
    pub repository_store: u64,
}

pub fn repository_artifact_checksum(bytes: &[u8]) -> u64 {
    checksum(bytes)
}

pub fn repository_artifact_file_checksum(
    path: impl AsRef<Path>,
) -> Result<u64, RepositorySnapshotError> {
    checksum_file(path.as_ref())
}

pub fn repository_identity_for_facts(
    facts: &RepositoryFacts,
    repository_store_checksum: u64,
) -> u64 {
    repository_identity_from_checksum(facts, repository_store_checksum)
}

pub fn publish_repository_snapshot(
    manifest_path: impl AsRef<Path>,
    request: PublishRepositorySnapshot<'_>,
) -> Result<RepositorySnapshotManifest, RepositorySnapshotError> {
    let manifest_path = manifest_path.as_ref();
    let root = root(manifest_path);
    let store_path = root.join(request.repository_store_file);
    let store_checksum = checksum_file(&store_path)?;
    let store = RepositoryStore::open(&store_path)?;
    let facts = store.materialize_facts()?;
    let compiled = compile_repository_facts(&facts)?;
    let repository_id = repository_identity_from_checksum(&facts, store_checksum);
    publish_manifest(
        manifest_path,
        request,
        &compiled,
        repository_id,
        store_checksum,
    )
}

pub fn publish_precompiled_repository_snapshot(
    manifest_path: impl AsRef<Path>,
    request: PublishRepositorySnapshot<'_>,
    compiled: &CompiledRepository,
    facts: &RepositoryFacts,
    checksums: RepositoryArtifactChecksums,
) -> Result<RepositorySnapshotManifest, RepositorySnapshotError> {
    let repository_id = repository_identity_from_checksum(facts, checksums.repository_store);
    publish_precompiled_repository_snapshot_with_identity(
        manifest_path,
        request,
        compiled,
        repository_id,
        checksums,
    )
}

pub fn publish_precompiled_repository_snapshot_with_identity(
    manifest_path: impl AsRef<Path>,
    request: PublishRepositorySnapshot<'_>,
    compiled: &CompiledRepository,
    repository_id: u64,
    checksums: RepositoryArtifactChecksums,
) -> Result<RepositorySnapshotManifest, RepositorySnapshotError> {
    let manifest_path = manifest_path.as_ref();
    let root = root(manifest_path);
    let store_path = root.join(request.repository_store_file);
    compare(
        "repository_store_checksum",
        checksums.repository_store,
        checksum_file(&store_path)?,
    )?;
    let store = RepositoryStore::open(&store_path)?;
    compare(
        "repository_store_node_count",
        u64::from(compiled.dataset.node_count),
        u64::from(store.node_count()),
    )?;
    publish_manifest(
        manifest_path,
        request,
        compiled,
        repository_id,
        checksums.repository_store,
    )
}

fn publish_manifest(
    manifest_path: &Path,
    request: PublishRepositorySnapshot<'_>,
    compiled: &CompiledRepository,
    repository_id: u64,
    repository_store_checksum: u64,
) -> Result<RepositorySnapshotManifest, RepositorySnapshotError> {
    let root = root(manifest_path);
    let graph = GraphSnapshot::open(root.join(request.graph_manifest_file))?;
    let graph_manifest_checksum = checksum(&fs::read(root.join(request.graph_manifest_file))?);
    let snapshot_id = derive_repository_snapshot_id(
        repository_id,
        graph.snapshot_id(),
        repository_store_checksum,
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
        unresolved_count: compiled.unresolved.len() as u64,
        graph_snapshot_id: graph.snapshot_id(),
        graph_manifest_checksum,
        repository_store_checksum,
        graph_manifest_file: request.graph_manifest_file.to_path_buf(),
        repository_store_file: request.repository_store_file.to_path_buf(),
    };
    validate_compiled_components(&manifest, &graph, compiled, repository_id)?;
    write_immutable(manifest_path, manifest.encode()?.as_bytes())?;
    Ok(manifest)
}

fn root(path: &Path) -> &Path {
    path.parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn checksum_file(path: &Path) -> Result<u64, RepositorySnapshotError> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut hasher = StableHasher::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            return Ok(hasher.finish());
        }
        hasher.update(&buffer[..count]);
    }
}

pub fn derive_repository_snapshot_id(
    repository_id: u64,
    graph_snapshot_id: u64,
    repository_store_checksum: u64,
    adapter_name: &str,
    adapter_version: &str,
) -> u64 {
    let mut hasher = StableHasher::new();
    hasher.update(b"arcana-repository-snapshot-v2");
    for value in [repository_id, graph_snapshot_id, repository_store_checksum] {
        hasher.update(&value.to_le_bytes());
    }
    hasher.update(adapter_name.as_bytes());
    hasher.update(&[0]);
    hasher.update(adapter_version.as_bytes());
    hasher.finish()
}

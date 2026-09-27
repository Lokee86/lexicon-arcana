use std::fs;
use std::io;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use arcana::repository::{
    CompiledRepositoryGraph, PublishRepositorySnapshot, RepositoryArtifactChecksums,
    RepositoryFacts, compile_repository_graph, publish_graph_repository_snapshot_with_identity,
    repository_artifact_file_checksum, repository_identity_for_facts,
};
use arcana::repository_store::{RepositoryStoreWrite, write_repository_store};
use arcana::snapshot::publish_snapshot;
use arcana::synthetic::GraphDataset;

use crate::cli_commands::CliCommandError;

const REPOSITORY_STORE_FILE: &str = "repository.arcana";

pub(crate) fn write_compiled_owned(
    output: &Path,
    facts: RepositoryFacts,
    adapter_name: &str,
    adapter_version: &str,
) -> Result<String, CliCommandError> {
    let (store_checksum, store_write) = write_store(output, &facts)?;
    let repository_id = repository_identity_for_facts(&facts, store_checksum);
    let unresolved_count = facts.unresolved.len();
    let graph_started = Instant::now();
    let graph = compile_repository_graph(&facts)?;
    if std::env::var_os("ARCANA_SYNC_PROFILE").is_some() {
        eprintln!(
            "arcana sync profile: phase=graph-compile elapsed_ms={:.3} nodes={} edges={} unresolved={}",
            graph_started.elapsed().as_secs_f64() * 1000.0,
            graph.dataset.node_count,
            graph.dataset.edges.len(),
            unresolved_count,
        );
    }
    write_graph(output, &graph.dataset)?;
    publish_graph_with_identity(
        output,
        &graph,
        repository_id,
        store_checksum,
        adapter_name,
        adapter_version,
        store_write,
    )?;
    import_summary(
        output,
        graph.dataset.node_count,
        graph.dataset.edges.len(),
        unresolved_count,
    )
}

pub(crate) fn write_repository_metadata_graph_owned(
    output: &Path,
    facts: RepositoryFacts,
    graph: &CompiledRepositoryGraph,
    adapter_name: &str,
    adapter_version: &str,
) -> Result<(), CliCommandError> {
    let (store_checksum, store_write) = write_store(output, &facts)?;
    let repository_id = repository_identity_for_facts(&facts, store_checksum);
    publish_graph_with_identity(
        output,
        graph,
        repository_id,
        store_checksum,
        adapter_name,
        adapter_version,
        store_write,
    )
}

fn write_graph(output: &Path, dataset: &GraphDataset) -> Result<(), CliCommandError> {
    let graph_path = output.join("graph.arcana");
    arcana::storage::write_packed(&graph_path, dataset)?;
    publish_snapshot(
        output.join("graph.manifest"),
        "graph.arcana",
        None,
        timestamp()?,
    )?;
    Ok(())
}

fn publish_graph_with_identity(
    output: &Path,
    graph: &CompiledRepositoryGraph,
    repository_id: u64,
    store_checksum: u64,
    adapter_name: &str,
    adapter_version: &str,
    store_write: RepositoryStoreWrite,
) -> Result<(), CliCommandError> {
    publish_graph_repository_snapshot_with_identity(
        output.join("repository.manifest"),
        publish_request(adapter_name, adapter_version)?,
        graph,
        repository_id,
        RepositoryArtifactChecksums {
            repository_store: store_checksum,
        },
        store_write,
    )?;
    Ok(())
}

fn write_store(
    output: &Path,
    facts: &RepositoryFacts,
) -> Result<(u64, RepositoryStoreWrite), CliCommandError> {
    let path = output.join(REPOSITORY_STORE_FILE);
    let store_write = write_repository_store(&path, facts)?;
    Ok((repository_artifact_file_checksum(&path)?, store_write))
}

fn publish_request<'a>(
    adapter_name: &'a str,
    adapter_version: &'a str,
) -> Result<PublishRepositorySnapshot<'a>, CliCommandError> {
    Ok(PublishRepositorySnapshot {
        graph_manifest_file: Path::new("graph.manifest"),
        repository_store_file: Path::new(REPOSITORY_STORE_FILE),
        adapter_name,
        adapter_version,
        created_unix_seconds: timestamp()?,
    })
}

fn import_summary(
    output: &Path,
    node_count: u32,
    edge_count: usize,
    unresolved_count: usize,
) -> Result<String, CliCommandError> {
    let graph_size = fs::metadata(output.join("graph.arcana"))?.len();
    let metadata_size = [
        REPOSITORY_STORE_FILE,
        "graph.manifest",
        "repository.manifest",
    ]
    .iter()
    .map(|file| fs::metadata(output.join(file)).map(|metadata| metadata.len()))
    .collect::<Result<Vec<_>, _>>()?
    .into_iter()
    .sum::<u64>();
    Ok(format!(
        "imported facts: nodes={} edges={} unresolved={} graph.arcana={} bytes metadata={} bytes total={} bytes\n",
        node_count,
        edge_count,
        unresolved_count,
        graph_size,
        metadata_size,
        graph_size + metadata_size
    ))
}

pub(crate) fn timestamp() -> Result<u64, CliCommandError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| io::Error::other(error).into())
}

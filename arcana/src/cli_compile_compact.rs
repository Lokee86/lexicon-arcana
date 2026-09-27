use std::path::Path;
use std::time::Instant;

use arcana::repository::{
    CompiledRepositoryGraph, compile_compact_repository_graph, repository_artifact_file_checksum,
};
use arcana::repository_store::{
    CompactRepositoryBuild, RepositoryStoreWrite, write_repository_store_compact,
};

use crate::cli_commands::CliCommandError;
use crate::cli_compile::{
    REPOSITORY_STORE_FILE, import_summary, publish_graph_with_identity, write_graph,
};

pub(crate) fn write_compiled_compact_owned(
    output: &Path,
    repository: CompactRepositoryBuild,
    adapter_name: &str,
    adapter_version: &str,
) -> Result<String, CliCommandError> {
    let unresolved_count = repository.unresolved_count();
    let (store_checksum, store_write) = write_store(output, &repository)?;
    let repository_id = repository.repository_identity(store_checksum);

    let graph_started = Instant::now();
    let graph = compile_compact_repository_graph(&repository)?;
    if std::env::var_os("ARCANA_SYNC_PROFILE").is_some() {
        eprintln!(
            "arcana sync profile: phase=compact-graph-compile elapsed_ms={:.3} nodes={} edges={} unresolved={}",
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

pub(crate) fn write_repository_metadata_graph_compact_owned(
    output: &Path,
    repository: CompactRepositoryBuild,
    graph: &CompiledRepositoryGraph,
    adapter_name: &str,
    adapter_version: &str,
) -> Result<(), CliCommandError> {
    let (store_checksum, store_write) = write_store(output, &repository)?;
    let repository_id = repository.repository_identity(store_checksum);
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

fn write_store(
    output: &Path,
    repository: &CompactRepositoryBuild,
) -> Result<(u64, RepositoryStoreWrite), CliCommandError> {
    let path = output.join(REPOSITORY_STORE_FILE);
    let store_write = write_repository_store_compact(&path, repository)?;
    Ok((repository_artifact_file_checksum(&path)?, store_write))
}

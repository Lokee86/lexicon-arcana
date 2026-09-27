use std::fs;
use std::path::Path;

use arcana::repository::{
    CompactIncrementalUpdate, IncrementalUpdate, RepositoryFacts, RepositorySnapshot,
    plan_file_update,
};
use arcana::snapshot::{publish_snapshot, write_overlay};
use arcana::storage::PackedGraph;

use crate::cli::UpdateFactsCommand;
use crate::cli_commands::CliCommandError;
use crate::cli_compile::{timestamp, write_repository_metadata_graph_owned};
use crate::cli_compile_compact::write_repository_metadata_graph_compact_owned;

pub fn run_update_facts(command: &UpdateFactsCommand) -> Result<String, CliCommandError> {
    if command.output.try_exists()? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "output directory already exists: {}",
                command.output.display()
            ),
        )
        .into());
    }
    let source = RepositorySnapshot::open(&command.base)?;
    let replacement = RepositoryFacts::parse(&fs::read_to_string(&command.facts)?)?;
    let packed_base = source.materialize_base_dataset()?;
    let update = plan_file_update(source.facts(), &replacement, &command.changed, &packed_base)?;
    fs::create_dir(&command.output)?;
    let result = write_update(
        &command.output,
        &source.base_graph_path(),
        update,
        &source.manifest().adapter_name,
        &source.manifest().adapter_version,
    );
    if result.is_err() {
        let _ = fs::remove_dir_all(&command.output);
    }
    result
}

pub(crate) fn write_update(
    output: &Path,
    base_graph_path: &Path,
    update: IncrementalUpdate,
    adapter_name: &str,
    adapter_version: &str,
) -> Result<String, CliCommandError> {
    let changed_file_count = update.changed_file_count();
    let summary = write_graph_update(output, base_graph_path, &update.changes, changed_file_count)?;
    write_repository_metadata_graph_owned(
        output,
        update.facts,
        &update.graph,
        adapter_name,
        adapter_version,
    )?;
    Ok(summary)
}

pub(crate) fn write_compact_update(
    output: &Path,
    base_graph_path: &Path,
    update: CompactIncrementalUpdate,
    adapter_name: &str,
    adapter_version: &str,
) -> Result<String, CliCommandError> {
    let changed_file_count = update.changed_file_count();
    let summary = write_graph_update(output, base_graph_path, &update.changes, changed_file_count)?;
    write_repository_metadata_graph_compact_owned(
        output,
        update.repository,
        &update.graph,
        adapter_name,
        adapter_version,
    )?;
    Ok(summary)
}

fn write_graph_update(
    output: &Path,
    base_graph_path: &Path,
    changes: &arcana::snapshot::OverlayChanges,
    changed_file_count: usize,
) -> Result<String, CliCommandError> {
    let added_edges = changes.added.len();
    let removed_edges = changes.removed.len();
    fs::copy(base_graph_path, output.join("graph.arcana"))?;
    let base = PackedGraph::open(output.join("graph.arcana"))?;
    let overlay_file = if changes.added.is_empty() && changes.removed.is_empty() {
        None
    } else {
        write_overlay(output.join("overlay.arcana"), &base, changes)?;
        Some(Path::new("overlay.arcana"))
    };
    publish_snapshot(
        output.join("graph.manifest"),
        "graph.arcana",
        overlay_file,
        timestamp()?,
    )?;
    Ok(format!(
        "updated facts: changed_files={} added_edges={} removed_edges={} overlay={}\n",
        changed_file_count,
        added_edges,
        removed_edges,
        overlay_file.is_some()
    ))
}

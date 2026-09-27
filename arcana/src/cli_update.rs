use std::fs;
use std::path::Path;

use arcana::repository::{
    IncrementalUpdate, RepositoryFacts, RepositorySnapshot, plan_file_update,
};
use arcana::snapshot::{publish_snapshot, write_overlay};
use arcana::storage::PackedGraph;

use crate::cli::UpdateFactsCommand;
use crate::cli_commands::CliCommandError;
use crate::cli_compile::{timestamp, write_repository_metadata_graph_owned};

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
    let added_edges = update.changes.added.len();
    let removed_edges = update.changes.removed.len();

    fs::copy(base_graph_path, output.join("graph.arcana"))?;
    let base = PackedGraph::open(output.join("graph.arcana"))?;
    let overlay_file = if update.changes.added.is_empty() && update.changes.removed.is_empty() {
        None
    } else {
        write_overlay(output.join("overlay.arcana"), &base, &update.changes)?;
        Some(Path::new("overlay.arcana"))
    };
    publish_snapshot(
        output.join("graph.manifest"),
        "graph.arcana",
        overlay_file,
        timestamp()?,
    )?;
    write_repository_metadata_graph_owned(
        output,
        update.facts,
        &update.graph,
        adapter_name,
        adapter_version,
    )?;
    Ok(format!(
        "updated facts: changed_files={} added_edges={} removed_edges={} overlay={}\n",
        changed_file_count,
        added_edges,
        removed_edges,
        overlay_file.is_some()
    ))
}

use std::error::Error as StdError;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use arcana::lexicon::{
    CompactLexiconSnapshot, LexiconSnapshotMetadata, load_compact, load_compact_delta,
};
use arcana::repository::{
    IncrementalError, RepositorySnapshot, RepositorySnapshotError, RepositoryUpdateBase,
    plan_compact_delta_edge_changes_from_store, repository_artifact_file_checksum,
};
use arcana::repository_store::rewrite_repository_store;

use crate::cli_commands::CliCommandError;
use crate::cli_compile::{REPOSITORY_STORE_FILE, publish_incremental_graph_with_identity};
use crate::cli_compile_compact::write_compiled_compact_owned;
use crate::cli_update::write_graph_update;

use super::cli_sync::{SyncError, snapshot_directory};

pub(super) struct SnapshotWrite {
    pub(super) mode: &'static str,
    pub(super) reason: Option<String>,
    pub(super) compatibility_warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RebuildReason {
    NoPreviousGeneration,
    NoUsablePreviousGeneration,
    CurrentGenerationInvalid,
    UnsupportedCurrentManifestVersion(u64),
    PreviousLexiconMetadataUnavailable,
    SharedObjectsChanged,
    NoChangedPaths,
    UnsupportedPreviousManifestVersion(u64),
    PreviousGenerationInvalid,
    ChangedNodeSet,
}

impl RebuildReason {
    pub(super) fn label(self) -> String {
        match self {
            Self::NoPreviousGeneration => "no-previous-generation".to_owned(),
            Self::NoUsablePreviousGeneration => "no-usable-previous-generation".to_owned(),
            Self::CurrentGenerationInvalid => "current-generation-invalid".to_owned(),
            Self::UnsupportedCurrentManifestVersion(version) => {
                format!("unsupported-current-manifest-v{version}")
            }
            Self::PreviousLexiconMetadataUnavailable => {
                "previous-lexicon-metadata-unavailable".to_owned()
            }
            Self::SharedObjectsChanged => "shared-objects-changed".to_owned(),
            Self::NoChangedPaths => "no-changed-paths".to_owned(),
            Self::UnsupportedPreviousManifestVersion(version) => {
                format!("unsupported-previous-manifest-v{version}")
            }
            Self::PreviousGenerationInvalid => "previous-generation-invalid".to_owned(),
            Self::ChangedNodeSet => "changed-node-set".to_owned(),
        }
    }
}

#[derive(Debug)]
pub(super) enum SyncPlan {
    Existing,
    Rebuild(RebuildReason),
    Incremental(IncrementalPlan),
}

#[derive(Debug)]
pub(super) struct IncrementalPlan {
    previous_manifest: PathBuf,
    changed_paths: Vec<String>,
}

pub(super) fn read_compatibility_warnings(output: &Path) -> Result<Vec<String>, SyncError> {
    match fs::read_to_string(output.join("compatibility.warnings")) {
        Ok(contents) => Ok(contents
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn plan_snapshot(
    lexicon_root: &Path,
    state: &Path,
    output: &Path,
    previous_id: Option<&str>,
    current: &LexiconSnapshotMetadata,
) -> Result<SyncPlan, SyncError> {
    if output.try_exists()? {
        return plan_existing_output(output, current.id());
    }

    let Some(previous_id) = previous_id else {
        return Ok(SyncPlan::Rebuild(RebuildReason::NoPreviousGeneration));
    };
    if previous_id == current.id() {
        return Ok(SyncPlan::Rebuild(RebuildReason::NoUsablePreviousGeneration));
    }

    let previous_directory = snapshot_directory(state, previous_id)?;
    let previous_manifest = previous_directory.join("repository.manifest");
    if !previous_manifest.try_exists()? {
        return Ok(SyncPlan::Rebuild(RebuildReason::NoUsablePreviousGeneration));
    }

    let previous_lexicon = match LexiconSnapshotMetadata::load(lexicon_root, previous_id) {
        Ok(metadata) => metadata,
        Err(error) if has_unexpected_io(&error) => return Err(error.into()),
        Err(_) => {
            return Ok(SyncPlan::Rebuild(
                RebuildReason::PreviousLexiconMetadataUnavailable,
            ));
        }
    };

    if current.shared_objects_changed(&previous_lexicon) {
        return Ok(SyncPlan::Rebuild(RebuildReason::SharedObjectsChanged));
    }

    let changes = current.changed_paths(&previous_lexicon);
    let mut changed_paths = changes.added;
    changed_paths.extend(changes.changed);
    changed_paths.extend(changes.removed);
    changed_paths.sort_unstable();
    changed_paths.dedup();
    if changed_paths.is_empty() {
        return Ok(SyncPlan::Rebuild(RebuildReason::NoChangedPaths));
    }

    let previous_arcana = match RepositoryUpdateBase::open(&previous_manifest) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return Ok(SyncPlan::Rebuild(classify_repository_error(error, false)?));
        }
    };
    if let Err(error) = previous_arcana.open_incremental_store() {
        return Ok(SyncPlan::Rebuild(classify_repository_error(error, false)?));
    }

    Ok(SyncPlan::Incremental(IncrementalPlan {
        previous_manifest,
        changed_paths,
    }))
}

pub(super) fn build_snapshot(
    lexicon_root: &Path,
    state: &Path,
    output: &Path,
    current: &LexiconSnapshotMetadata,
    plan: SyncPlan,
) -> Result<SnapshotWrite, SyncError> {
    let temp = state.join("snapshots").join(format!(
        ".{}.tmp-{}",
        current.id().trim_start_matches("sha256:"),
        std::process::id()
    ));
    if temp.try_exists()? {
        fs::remove_dir_all(&temp)?;
    }
    fs::create_dir(&temp)?;
    let result = match write_snapshot(lexicon_root, &temp, current, plan) {
        Ok(result) => result,
        Err(error) => {
            let _ = fs::remove_dir_all(&temp);
            return Err(error);
        }
    };
    fs::write(temp.join("lexicon.snapshot"), format!("{}\n", current.id()))?;
    if !result.compatibility_warnings.is_empty() {
        fs::write(
            temp.join("compatibility.warnings"),
            result.compatibility_warnings.join("\n") + "\n",
        )?;
    }
    fs::rename(&temp, output)?;
    Ok(result)
}

fn plan_existing_output(output: &Path, current_id: &str) -> Result<SyncPlan, SyncError> {
    let source = match fs::read_to_string(output.join("lexicon.snapshot")) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(SyncPlan::Rebuild(RebuildReason::CurrentGenerationInvalid));
        }
        Err(error) => return Err(error.into()),
    };
    if source.trim() != current_id {
        return Ok(SyncPlan::Rebuild(RebuildReason::CurrentGenerationInvalid));
    }

    match RepositorySnapshot::open(output.join("repository.manifest")) {
        Ok(_) => Ok(SyncPlan::Existing),
        Err(error) => Ok(SyncPlan::Rebuild(classify_repository_error(error, true)?)),
    }
}

fn write_snapshot(
    lexicon_root: &Path,
    output: &Path,
    current: &LexiconSnapshotMetadata,
    plan: SyncPlan,
) -> Result<SnapshotWrite, SyncError> {
    match plan {
        SyncPlan::Existing => Err(SyncError::InvalidState(
            "existing Arcana snapshot was sent to the build path".to_owned(),
        )),
        SyncPlan::Rebuild(reason) => rebuild_snapshot(lexicon_root, output, current, reason),
        SyncPlan::Incremental(plan) => {
            write_incremental_snapshot(lexicon_root, output, current, plan)
        }
    }
}

fn rebuild_snapshot(
    lexicon_root: &Path,
    output: &Path,
    current: &LexiconSnapshotMetadata,
    reason: RebuildReason,
) -> Result<SnapshotWrite, SyncError> {
    let current_snapshot = load_compact(lexicon_root, current.id())?;
    rebuild_loaded_snapshot(output, current_snapshot, reason)
}

fn write_incremental_snapshot(
    lexicon_root: &Path,
    output: &Path,
    current: &LexiconSnapshotMetadata,
    plan: IncrementalPlan,
) -> Result<SnapshotWrite, SyncError> {
    let previous_arcana = match RepositoryUpdateBase::open(&plan.previous_manifest) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let reason = classify_repository_error(error, false)?;
            return rebuild_snapshot(lexicon_root, output, current, reason);
        }
    };
    let base_graph_path = previous_arcana.base_graph_path();
    let mut base_store = match previous_arcana.open_incremental_store() {
        Ok(store) => store,
        Err(error) => {
            let reason = classify_repository_error(error, false)?;
            return rebuild_snapshot(lexicon_root, output, current, reason);
        }
    };

    let delta = load_compact_delta(lexicon_root, current, &plan.changed_paths, &mut base_store)?;
    let changes = match plan_compact_delta_edge_changes_from_store(
        &mut base_store,
        &delta.repository,
        &plan.changed_paths,
    ) {
        Ok(changes) => changes,
        Err(IncrementalError::NodeSetChanged { .. }) => {
            return rebuild_snapshot(lexicon_root, output, current, RebuildReason::ChangedNodeSet);
        }
        Err(IncrementalError::Store(error)) if !has_unexpected_io(&error) => {
            return rebuild_snapshot(
                lexicon_root,
                output,
                current,
                RebuildReason::PreviousGenerationInvalid,
            );
        }
        Err(error) => return Err(error.into()),
    };

    let rewrite = rewrite_repository_store(
        output.join(REPOSITORY_STORE_FILE),
        &mut base_store,
        &plan.changed_paths,
        &delta.repository,
    )
    .map_err(CliCommandError::from)?;
    let store_checksum = repository_artifact_file_checksum(output.join(REPOSITORY_STORE_FILE))?;
    let repository_id = rewrite.repository_identity(store_checksum);
    let current_id = delta.metadata.id().to_owned();
    let compatibility_warnings = delta.compatibility_warnings;
    let changed_file_count = plan.changed_paths.len();
    drop(base_store);
    drop(previous_arcana);

    write_graph_update(output, &base_graph_path, &changes, changed_file_count)?;
    publish_incremental_graph_with_identity(
        output,
        repository_id,
        store_checksum,
        "lexicon",
        &current_id,
        rewrite.write,
    )?;

    Ok(SnapshotWrite {
        mode: "overlay",
        reason: None,
        compatibility_warnings,
    })
}

fn rebuild_loaded_snapshot(
    output: &Path,
    current: CompactLexiconSnapshot,
    reason: RebuildReason,
) -> Result<SnapshotWrite, SyncError> {
    let current_id = current.metadata.id().to_owned();
    let compatibility_warnings = current.compatibility_warnings;
    write_compiled_compact_owned(output, current.repository, "lexicon", &current_id)?;
    Ok(SnapshotWrite {
        mode: "rebuild",
        reason: Some(reason.label()),
        compatibility_warnings,
    })
}

fn classify_repository_error(
    error: RepositorySnapshotError,
    current_generation: bool,
) -> Result<RebuildReason, SyncError> {
    match error {
        RepositorySnapshotError::UnsupportedManifestVersion(version) => {
            if current_generation {
                Ok(RebuildReason::UnsupportedCurrentManifestVersion(version))
            } else {
                Ok(RebuildReason::UnsupportedPreviousManifestVersion(version))
            }
        }
        error if has_unexpected_io(&error) => Err(error.into()),
        _ if current_generation => Ok(RebuildReason::CurrentGenerationInvalid),
        _ => Ok(RebuildReason::PreviousGenerationInvalid),
    }
}

fn has_unexpected_io(error: &(dyn StdError + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(source) = current {
        if let Some(error) = source.downcast_ref::<io::Error>() {
            return error.kind() != io::ErrorKind::NotFound;
        }
        current = source.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::has_unexpected_io;
    use std::io;

    #[test]
    fn only_not_found_io_is_treated_as_recoverable_generated_state() {
        assert!(!has_unexpected_io(&io::Error::from(
            io::ErrorKind::NotFound
        )));
        assert!(has_unexpected_io(&io::Error::from(
            io::ErrorKind::PermissionDenied
        )));
    }
}

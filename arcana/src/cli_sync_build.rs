use std::fs;
use std::io;
use std::path::Path;

use arcana::lexicon::{CompactLexiconSnapshot, LexiconSnapshotMetadata, load_compact};
use arcana::repository::{RepositoryUpdateBase, plan_verified_compact_snapshot_update_from_store};

use crate::cli_compile_compact::write_compiled_compact_owned;
use crate::cli_update::write_compact_update;

use super::cli_sync::{SyncError, snapshot_directory};

pub(super) struct SnapshotWrite {
    pub(super) mode: &'static str,
    pub(super) compatibility_warnings: Vec<String>,
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

pub(super) fn build_snapshot(
    lexicon_root: &Path,
    state: &Path,
    output: &Path,
    previous_id: Option<&str>,
    current: &LexiconSnapshotMetadata,
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
    let result = match write_snapshot(lexicon_root, state, &temp, previous_id, current) {
        Ok(result) => result,
        Err(error) => {
            let _ = fs::remove_dir_all(&temp);
            return Err(error);
        }
    };
    fs::write(
        temp.join("lexicon.snapshot"),
        format!(
            "{}
",
            current.id()
        ),
    )?;
    if !result.compatibility_warnings.is_empty() {
        fs::write(
            temp.join("compatibility.warnings"),
            result.compatibility_warnings.join(
                "
",
            ) + "
",
        )?;
    }
    fs::rename(&temp, output)?;
    Ok(result)
}

fn write_snapshot(
    lexicon_root: &Path,
    state: &Path,
    output: &Path,
    previous_id: Option<&str>,
    current: &LexiconSnapshotMetadata,
) -> Result<SnapshotWrite, SyncError> {
    let current_snapshot = load_compact(lexicon_root, current.id())?;

    if let Some(previous_id) = previous_id.filter(|id| *id != current.id()) {
        let previous_directory = snapshot_directory(state, previous_id)?;
        let previous_manifest = previous_directory.join("repository.manifest");
        if previous_manifest.is_file()
            && let Ok(previous_lexicon) = LexiconSnapshotMetadata::load(lexicon_root, previous_id)
        {
            if current.shared_objects_changed(&previous_lexicon) {
                return rebuild_loaded_snapshot(output, current_snapshot);
            }

            let changes = current.changed_paths(&previous_lexicon);
            let mut changed_paths = changes.added;
            changed_paths.extend(changes.changed);
            changed_paths.extend(changes.removed);
            changed_paths.sort_unstable();
            changed_paths.dedup();

            if !changed_paths.is_empty()
                && let Ok(previous_arcana) = RepositoryUpdateBase::open(&previous_manifest)
            {
                let packed_base = previous_arcana.materialize_base_dataset()?;
                if let Ok(mut base_store) = previous_arcana.open_incremental_store()
                    && let Ok(plan) = plan_verified_compact_snapshot_update_from_store(
                        &mut base_store,
                        &current_snapshot.repository,
                        &changed_paths,
                        &packed_base,
                    )
                {
                    let base_graph_path = previous_arcana.base_graph_path();
                    let current_id = current_snapshot.metadata.id().to_owned();
                    let compatibility_warnings = current_snapshot.compatibility_warnings.clone();
                    drop(base_store);
                    drop(packed_base);
                    drop(previous_arcana);
                    let update = plan.finish(current_snapshot.repository);
                    write_compact_update(output, &base_graph_path, update, "lexicon", &current_id)?;
                    return Ok(SnapshotWrite {
                        mode: "overlay",
                        compatibility_warnings,
                    });
                }
                return rebuild_loaded_snapshot(output, current_snapshot);
            }
        }
    }

    rebuild_loaded_snapshot(output, current_snapshot)
}

fn rebuild_loaded_snapshot(
    output: &Path,
    current: CompactLexiconSnapshot,
) -> Result<SnapshotWrite, SyncError> {
    let current_id = current.metadata.id().to_owned();
    let compatibility_warnings = current.compatibility_warnings;
    write_compiled_compact_owned(output, current.repository, "lexicon", &current_id)?;
    Ok(SnapshotWrite {
        mode: "rebuild",
        compatibility_warnings,
    })
}

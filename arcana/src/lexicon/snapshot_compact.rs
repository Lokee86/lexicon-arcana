use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;

use super::snapshot::read_manifest;
use super::snapshot_compact_visit::{
    visit_node_pass, visit_node_pass_selected, visit_relation_pass, visit_relation_pass_selected,
};
use super::snapshot_support::storage_root;
use super::stream_compact::CompactPass;
use super::{LexiconSnapshotError, LexiconSnapshotMetadata};
use crate::repository::normalize_repository_path;
use crate::repository_store::{
    CompactRepositoryBuild, CompactRepositoryDelta, RepositoryStoreFile,
};
#[cfg(test)]
use crate::repository_store::{RepositoryStoreWrite, write_repository_store_compact};

#[doc(hidden)]
pub struct CompactLexiconSnapshot {
    pub metadata: LexiconSnapshotMetadata,
    pub repository: CompactRepositoryBuild,
    pub compatibility_warnings: Vec<String>,
    pub direct_v2: bool,
}

#[derive(Debug)]
#[doc(hidden)]
pub struct CompactLexiconDelta {
    pub metadata: LexiconSnapshotMetadata,
    pub repository: CompactRepositoryDelta,
    pub changed_paths: Vec<String>,
    pub compatibility_warnings: Vec<String>,
    pub direct_v2: bool,
}

impl CompactLexiconDelta {
    pub fn owned_node_keys(&self) -> Vec<crate::repository::NodeKey> {
        self.repository.owned_node_keys(&self.changed_paths)
    }
}

#[doc(hidden)]
pub fn load_compact(
    root: impl AsRef<Path>,
    id: &str,
) -> Result<CompactLexiconSnapshot, LexiconSnapshotError> {
    let storage = storage_root(root.as_ref());
    let (manifest, metadata) = read_manifest(&storage, id)?;
    let mut pass = CompactPass::new();

    let node_started = Instant::now();
    let direct_v2 = visit_node_pass(&storage, &manifest, &mut pass)?;
    profile("compact-node-pass", node_started.elapsed());

    pass.finish_node_pass()?;

    let relation_started = Instant::now();
    visit_relation_pass(&storage, &manifest, &mut pass)?;
    profile("compact-relation-pass", relation_started.elapsed());

    let finish_started = Instant::now();
    let (repository, compatibility_warnings) = pass.finish()?;
    profile("compact-build-finish", finish_started.elapsed());

    Ok(CompactLexiconSnapshot {
        metadata,
        repository,
        compatibility_warnings,
        direct_v2,
    })
}

#[doc(hidden)]
pub fn load_compact_delta(
    root: impl AsRef<Path>,
    current: &LexiconSnapshotMetadata,
    changed_paths: &[String],
    base: &mut RepositoryStoreFile,
) -> Result<CompactLexiconDelta, LexiconSnapshotError> {
    let storage = storage_root(root.as_ref());
    let (manifest, metadata) = read_manifest(&storage, current.id())?;
    if &metadata != current {
        return Err(LexiconSnapshotError::MetadataMismatch("snapshot metadata"));
    }

    let selected_paths = changed_paths
        .iter()
        .map(|path| {
            normalize_repository_path(path).map_err(|_| LexiconSnapshotError::InvalidPath {
                field: "file",
                path: path.clone(),
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;

    let mut pass = CompactPass::new();
    let node_started = Instant::now();
    let direct_v2 = visit_node_pass_selected(&storage, &manifest, &selected_paths, &mut pass)?;
    profile("incremental-object-node-load", node_started.elapsed());

    pass.finish_delta_node_pass()?;

    let relation_started = Instant::now();
    visit_relation_pass_selected(&storage, &manifest, &selected_paths, &mut pass, base)?;
    profile(
        "incremental-object-relation-load",
        relation_started.elapsed(),
    );

    let finish_started = Instant::now();
    let (repository, compatibility_warnings) = pass.finish_delta()?;
    profile("incremental-delta-finish", finish_started.elapsed());

    Ok(CompactLexiconDelta {
        metadata,
        repository,
        changed_paths: selected_paths.into_iter().collect(),
        compatibility_warnings,
        direct_v2,
    })
}

fn profile(phase: &str, elapsed: std::time::Duration) {
    if std::env::var_os("ARCANA_SYNC_PROFILE").is_some() {
        eprintln!(
            "arcana sync profile: phase={phase} elapsed_ms={:.3}",
            elapsed.as_secs_f64() * 1000.0,
        );
    }
}

#[cfg(test)]
pub(crate) fn load_and_write_compact(
    root: impl AsRef<Path>,
    id: &str,
    path: impl AsRef<Path>,
) -> Result<(CompactLexiconSnapshot, RepositoryStoreWrite), LexiconSnapshotError> {
    let snapshot = load_compact(root, id)?;
    let write = write_repository_store_compact(path, &snapshot.repository)?;
    Ok((snapshot, write))
}

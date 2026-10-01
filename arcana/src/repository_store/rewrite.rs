use std::collections::BTreeSet;
use std::path::Path;

use crate::repository::{FactOwnershipError, normalize_repository_path};

use super::rewrite_indexes::stage_indexes;
use super::rewrite_io::RewriteWorkspace;
use super::rewrite_output::write_rewritten_store;
use super::rewrite_ownership::stage_ownership;
use super::rewrite_records::{stage_edges, stage_nodes, stage_unresolved};
use super::rewrite_strings::RewriteStrings;
use super::{
    CompactRepositoryDelta, ContributionKindView, RepositoryStoreFile, RepositoryStoreWrite,
    RepositoryStoreWriteError,
};

#[derive(Default)]
pub(super) struct ChangedRecords {
    pub(super) nodes: BTreeSet<u64>,
    pub(super) edges: BTreeSet<u64>,
    pub(super) unresolved: BTreeSet<u64>,
}

#[derive(Debug)]
#[doc(hidden)]
pub struct RepositoryStoreRewrite {
    pub write: RepositoryStoreWrite,
    repository_key: Option<crate::repository::NodeKey>,
}

impl RepositoryStoreRewrite {
    pub fn repository_identity(&self, store_checksum: u64) -> u64 {
        self.repository_key
            .map(crate::repository::NodeKey::as_u64)
            .unwrap_or(store_checksum)
    }
}

#[doc(hidden)]
pub fn rewrite_repository_store(
    path: impl AsRef<Path>,
    base: &mut RepositoryStoreFile,
    changed_paths: &[String],
    delta: &CompactRepositoryDelta,
) -> Result<RepositoryStoreRewrite, RepositoryStoreWriteError> {
    let changed_paths = changed_paths
        .iter()
        .map(|path| normalize_repository_path(path).map_err(FactOwnershipError::InvalidPath))
        .collect::<Result<Vec<_>, _>>()?;
    verify_node_set(base, delta, &changed_paths)?;

    let changed = ChangedRecords::collect(base, &changed_paths)?;
    let work = RewriteWorkspace::new(path.as_ref())?;
    std::fs::create_dir(work.path("sort"))?;

    let mut strings = RewriteStrings::build(base, delta, &changed, &work)?;
    let nodes = stage_nodes(base, delta, &changed, &mut strings, &work)?;
    let mut edges = stage_edges(base, delta, &changed, &mut strings, &work)?;
    let mut unresolved = stage_unresolved(base, delta, &changed, &mut strings, &work)?;
    let ownership = stage_ownership(
        base,
        delta,
        &changed,
        &mut strings,
        &nodes,
        &mut edges,
        &mut unresolved,
        &work,
    )?;
    let indexes = stage_indexes(base, delta, &changed, &mut strings, &nodes, &work)?;

    let repository_key = nodes.repository_key;
    let write = write_rewritten_store(
        path.as_ref(),
        &strings,
        &nodes,
        &edges,
        &unresolved,
        &ownership,
        &indexes,
    )?;
    Ok(RepositoryStoreRewrite {
        write,
        repository_key,
    })
}

impl ChangedRecords {
    fn collect(
        base: &mut RepositoryStoreFile,
        paths: &[String],
    ) -> Result<Self, RepositoryStoreWriteError> {
        let mut changed = Self::default();
        for path in paths {
            let Some(owner) = base.find_ownership(path)? else {
                continue;
            };
            base.for_each_owned_contribution(owner, |kind, index| {
                match kind {
                    ContributionKindView::Node => {
                        changed.nodes.insert(index);
                    }
                    ContributionKindView::Edge => {
                        changed.edges.insert(index);
                    }
                    ContributionKindView::Unresolved => {
                        changed.unresolved.insert(index);
                    }
                }
                Ok(())
            })?;
        }
        Ok(changed)
    }
}

fn verify_node_set(
    base: &mut RepositoryStoreFile,
    delta: &CompactRepositoryDelta,
    paths: &[String],
) -> Result<(), RepositoryStoreWriteError> {
    if base.owned_node_keys(paths)? == delta.owned_node_keys(paths) {
        Ok(())
    } else {
        Err(RepositoryStoreWriteError::ReplacementNodeSetMismatch)
    }
}

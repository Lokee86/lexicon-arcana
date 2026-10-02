//! Metadata lifetime independent of a prepared graph.
use super::{CatalogueEntry, NodeKey};
use crate::repository_store::{RepositoryStoreFile, RepositoryStoreReadError};

/// An already validated immutable repository store with bounded page caching.
/// Consumers coordinate concurrent access; no graph or materialized catalogue is retained.
#[derive(Debug)]
pub struct RepositoryMetadataReader {
    pub(super) snapshot_id: u64,
    pub(super) store: RepositoryStoreFile,
}

impl RepositoryMetadataReader {
    pub const fn snapshot_id(&self) -> u64 {
        self.snapshot_id
    }

    pub fn lookup_by_key(
        &mut self,
        key: NodeKey,
    ) -> Result<Option<CatalogueEntry>, RepositoryStoreReadError> {
        match self.store.node_id(key)? {
            Some(id) => self.store.entry(id),
            None => Ok(None),
        }
    }

    /// Resident page payload, always bounded by the store's 4 MiB cache.
    pub fn cached_bytes(&self) -> usize {
        self.store.cached_bytes()
    }
}

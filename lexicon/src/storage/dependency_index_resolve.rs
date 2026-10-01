use super::{LanguageEntry, StorageError, Store};

impl Store {
    /// The only index resolution seam used by the planner and delta publisher.
    /// Indexed generations never deserialize stored fact objects.
    pub(super) fn index_for_snapshot(
        &self,
        snapshot: &str,
        entry: &LanguageEntry,
    ) -> Result<String, StorageError> {
        if !entry.dependency_index_id.is_empty() {
            return Ok(entry.dependency_index_id.clone());
        }
        match self.read_bootstrap(snapshot, entry)? {
            Some(index_id) => Ok(index_id),
            None => self.bootstrap_legacy_index(snapshot, entry),
        }
    }
}

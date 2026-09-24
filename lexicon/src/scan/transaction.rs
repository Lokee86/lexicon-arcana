use crate::{SnapshotManifest, StorageError, Store};

use super::PublicationTransaction;

impl Store {
    pub fn begin_scan_publication(
        &self,
        manifest: &SnapshotManifest,
        base_state_commit: &str,
        commit_required: bool,
    ) -> Result<PublicationTransaction, StorageError> {
        self.write_pending(base_state_commit, commit_required, manifest)?;
        let mut candidate = manifest.clone();
        candidate.state_commit.clear();
        Ok(PublicationTransaction {
            manifest: candidate,
            base_state_commit: base_state_commit.to_owned(),
            commit_required,
        })
    }

    pub fn finish_scan_publication(
        &self,
        transaction: PublicationTransaction,
        state_commit: &str,
    ) -> Result<String, StorageError> {
        let mut manifest = transaction.manifest;
        manifest.state_commit = state_commit.to_owned();
        let id = self.publish(&manifest)?;
        self.clear_pending()?;
        Ok(id)
    }
}

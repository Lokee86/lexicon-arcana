use std::fs;
use std::io;

use super::io::write_atomic;
use super::{PendingPublication, RecoveryOutcome, SnapshotManifest, StorageError, Store};

const PENDING_VERSION: u64 = 1;

impl Store {
    pub fn write_pending(
        &self,
        base_state_commit: &str,
        commit_required: bool,
        manifest: &SnapshotManifest,
    ) -> Result<(), StorageError> {
        let mut manifest = manifest.clone();
        manifest.state_commit.clear();
        let pending = PendingPublication {
            version: PENDING_VERSION,
            base_state_commit: base_state_commit.to_owned(),
            commit_required,
            manifest,
        };
        let mut bytes = serde_json::to_vec(&pending)?;
        bytes.push(b'\n');
        write_atomic(&self.pending_path(), &bytes)
    }

    pub fn pending(&self) -> Result<PendingPublication, StorageError> {
        let bytes = match fs::read(self.pending_path()) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(StorageError::NoPendingPublication);
            }
            Err(error) => return Err(error.into()),
        };
        let pending: PendingPublication = serde_json::from_slice(super::store::trim_ascii(&bytes))?;
        if pending.version != PENDING_VERSION {
            return Err(StorageError::UnsupportedPendingVersion(pending.version));
        }
        Ok(pending)
    }

    pub fn clear_pending(&self) -> Result<(), StorageError> {
        match fs::remove_file(self.pending_path()) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn recover_pending(
        &self,
        state_commit: Option<&str>,
    ) -> Result<RecoveryOutcome, StorageError> {
        let pending = match self.pending() {
            Ok(pending) => pending,
            Err(StorageError::NoPendingPublication) => return Ok(RecoveryOutcome::NoPending),
            Err(error) => return Err(error),
        };
        let head = state_commit.unwrap_or("");
        if pending.commit_required && (head.is_empty() || head == pending.base_state_commit) {
            self.clear_pending()?;
            return Ok(RecoveryOutcome::Discarded);
        }

        let mut manifest = pending.manifest;
        manifest.state_commit = head.to_owned();
        let id = self.publish(&manifest)?;
        self.clear_pending()?;
        Ok(RecoveryOutcome::Published(id))
    }
}

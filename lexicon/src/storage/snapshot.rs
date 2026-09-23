use super::digest::domain_id;
use super::{SNAPSHOT_VERSION, SnapshotManifest, StorageError};

const SNAPSHOT_DOMAIN: &[u8] = b"lexicon:snapshot:v1\0";

pub fn snapshot_bytes(manifest: &SnapshotManifest) -> Result<Vec<u8>, StorageError> {
    if manifest.state_commit.is_empty() {
        return Err(StorageError::InvalidObject("state_commit"));
    }
    let mut canonical = manifest.clone();
    canonical.version = SNAPSHOT_VERSION;
    serde_json::to_vec(&canonical).map_err(StorageError::from)
}

pub fn snapshot_id(manifest: &SnapshotManifest) -> Result<String, StorageError> {
    Ok(domain_id(SNAPSHOT_DOMAIN, &snapshot_bytes(manifest)?))
}

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::digest::domain_id;
use super::{LanguageEntry, StorageError};

pub(super) const VERSION: u64 = 1;
pub(super) const DOMAIN: &[u8] = b"lexicon:dependency-index:v1\0";
pub(super) const SHARDS: u8 = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct IndexRoot {
    pub version: u64,
    pub language_signature: String,
    pub files: BTreeMap<u8, String>,
    pub nodes: BTreeMap<u8, String>,
    pub references: BTreeMap<u8, String>,
    pub unresolved: BTreeMap<u8, String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub(super) struct FileTopology {
    pub forward: BTreeSet<String>,
    pub reverse: BTreeSet<String>,
    // Evidence retained for Phase 3's precise per-file incremental updates.
    pub nodes: BTreeSet<String>,
    pub referenced_nodes: BTreeSet<String>,
    pub unresolved_candidates: BTreeSet<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct LegacyBootstrap {
    pub version: u64,
    pub snapshot_id: String,
    pub language_signature: String,
    pub index_id: String,
}

pub(super) fn shard(key: &str) -> u8 {
    use sha2::{Digest, Sha256};
    Sha256::digest(key.as_bytes())[0] % SHARDS
}

pub(super) fn language_signature(entry: &LanguageEntry) -> Result<String, StorageError> {
    let mut canonical = entry.clone();
    canonical.dependency_index_id.clear();
    Ok(domain_id(DOMAIN, &serde_json::to_vec(&canonical)?))
}

use crate::facts::FactRecord;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub struct FactObject {
    pub version: u64,
    pub language: String,
    pub owner: String,
    pub source_content_id: String,
    pub adapter_version: String,
    pub schema_version: u64,
    pub analysis_config_id: String,
    pub records: Vec<FactRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub language: String,
    pub content_id: String,
    pub object_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageEntry {
    pub language: String,
    pub adapter_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub adapter_fingerprint: String,
    pub schema_version: u64,
    pub repository: String,
    pub analysis_config_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub shared_object_id: String,
    pub files: Option<Vec<FileEntry>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotManifest {
    pub version: u64,
    pub state_commit: String,
    pub languages: Option<Vec<LanguageEntry>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingPublication {
    pub version: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub base_state_commit: String,
    pub commit_required: bool,
    pub manifest: SnapshotManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryOutcome {
    NoPending,
    Discarded,
    Published(String),
}

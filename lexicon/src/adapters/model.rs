use std::path::PathBuf;

pub const ADAPTER_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdapterRequest {
    pub language: String,
    pub repository: PathBuf,
    pub output: PathBuf,
    pub changed_files: Vec<String>,
    pub removed_files: Vec<String>,
    pub workers: usize,
    pub shards: usize,
    pub merge_fan_in: usize,
}

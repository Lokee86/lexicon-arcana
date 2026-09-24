use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdapterRequest {
    pub language: String,
    pub repository: PathBuf,
    pub changed_files: Vec<String>,
    pub removed_files: Vec<String>,
    pub workers: usize,
    pub shards: usize,
    pub merge_fan_in: usize,
}

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AdapterMode {
    #[default]
    Full,
    Incremental,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdapterRequest {
    pub language: String,
    pub mode: AdapterMode,
    pub repository: PathBuf,
    pub changed_files: Vec<String>,
    pub removed_files: Vec<String>,
    pub workers: usize,
    pub shards: usize,
    pub merge_fan_in: usize,
}

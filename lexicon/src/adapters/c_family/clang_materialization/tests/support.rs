use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use serde_json::json;

pub(super) fn span(
    path: &str,
    start_line: u64,
    start_column: u64,
    end_line: u64,
    end_column: u64,
) -> serde_json::Value {
    json!({
        "path": path,
        "start_line": start_line,
        "start_column": start_column,
        "end_line": end_line,
        "end_column": end_column
    })
}

pub(super) struct TestDirectory {
    pub(super) path: PathBuf,
}

impl TestDirectory {
    pub(super) fn new(label: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lexicon-c-family-clang-materialize-{label}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

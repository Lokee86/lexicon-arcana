pub mod interstack;
pub mod scan_adapter;

use lexicon::SnapshotManifest;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct TestDirectory {
    pub path: PathBuf,
}

impl TestDirectory {
    pub fn new(name: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lexicon-rust-{name}-{}-{}",
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

#[allow(dead_code)]
pub fn manifest(state_commit: &str) -> SnapshotManifest {
    SnapshotManifest {
        version: 99,
        state_commit: state_commit.to_owned(),
        languages: Some(Vec::new()),
    }
}

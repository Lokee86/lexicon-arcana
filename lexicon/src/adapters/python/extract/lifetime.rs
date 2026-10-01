use std::sync::atomic::{AtomicU64, Ordering};

use super::super::model::SourceFile;

#[derive(Debug, Default)]
pub struct ExtractionMetrics {
    pub peak_retained_source_bytes: u64,
    pub peak_retained_files: u64,
    pub peak_retained_ast_files: u64,
    pub workers: u64,
    pub logical_shards: u64,
}

#[derive(Default)]
pub(super) struct LifetimeTracker {
    source_bytes: AtomicU64,
    peak_source_bytes: AtomicU64,
    files: AtomicU64,
    peak_files: AtomicU64,
    ast_files: AtomicU64,
    peak_ast_files: AtomicU64,
}

impl LifetimeTracker {
    pub(super) fn enter(&self, file: &SourceFile) {
        let retained = (file.bytes.len() + file.source.len() + file.lines.retained_bytes()) as u64;
        let source = self.source_bytes.fetch_add(retained, Ordering::Relaxed) + retained;
        self.peak_source_bytes.fetch_max(source, Ordering::Relaxed);

        let files = self.files.fetch_add(1, Ordering::Relaxed) + 1;
        self.peak_files.fetch_max(files, Ordering::Relaxed);

        if file.suite.is_some() {
            let asts = self.ast_files.fetch_add(1, Ordering::Relaxed) + 1;
            self.peak_ast_files.fetch_max(asts, Ordering::Relaxed);
        }
    }

    pub(super) fn exit(&self, file: &SourceFile) {
        let retained = (file.bytes.len() + file.source.len() + file.lines.retained_bytes()) as u64;
        self.source_bytes.fetch_sub(retained, Ordering::Relaxed);
        self.files.fetch_sub(1, Ordering::Relaxed);
        if file.suite.is_some() {
            self.ast_files.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub(super) fn metrics(&self, workers: usize, shards: usize) -> ExtractionMetrics {
        ExtractionMetrics {
            peak_retained_source_bytes: self.peak_source_bytes.load(Ordering::Relaxed),
            peak_retained_files: self.peak_files.load(Ordering::Relaxed),
            peak_retained_ast_files: self.peak_ast_files.load(Ordering::Relaxed),
            workers: workers as u64,
            logical_shards: shards as u64,
        }
    }
}

use super::dependency_index_read::IndexReader;
use super::{IncrementalScope, StorageError, Store};

impl Store {
    pub fn incremental_scope(
        &self,
        language: &str,
        roots: &[String],
    ) -> Result<IncrementalScope, StorageError> {
        self.incremental_scope_with_additions(language, roots, &[])
    }

    pub fn incremental_scope_with_additions(
        &self,
        language: &str,
        roots: &[String],
        additions: &[String],
    ) -> Result<IncrementalScope, StorageError> {
        let started = crate::perf::start();
        let (snapshot, manifest) = self.current()?;
        let entry = manifest.language(language).ok_or_else(|| {
            StorageError::Materialization(format!("snapshot has no {language} analysis"))
        })?;
        // No graph assembly or fact-object decoder in the planner. A
        // historical unindexed generation migrates once through a separate
        // snapshot-bound storage operation.
        let index_id = self.index_for_snapshot(&snapshot, entry)?;
        let mut reader = IndexReader::open(self, entry, &index_id)?;
        let result = reader.scope(language, roots, additions)?;
        if let Some(started) = started {
            crate::perf::emit(
                "scan.dependency_scope",
                started.elapsed(),
                &[
                    ("roots", roots.len() as u64),
                    ("emit_files", result.emit.len() as u64),
                    ("context_files", result.context.len() as u64),
                    ("full_required", u64::from(result.full_required)),
                ],
            );
        }
        Ok(result)
    }

    pub fn dependency_scope(
        &self,
        language: &str,
        roots: &[String],
    ) -> Result<(Vec<String>, Vec<String>), StorageError> {
        let scope = self.incremental_scope(language, roots)?;
        Ok((scope.emit, scope.context))
    }

    pub fn impacted_files(
        &self,
        language: &str,
        roots: &[String],
    ) -> Result<Vec<String>, StorageError> {
        Ok(self.incremental_scope(language, roots)?.emit)
    }

    pub fn direct_changes_require_full(
        &self,
        language: &str,
        roots: &[String],
    ) -> Result<bool, StorageError> {
        Ok(self.incremental_scope(language, roots)?.full_required)
    }
}

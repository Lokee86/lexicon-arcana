use std::collections::BTreeMap;

use crate::FactRecord;

use super::dependency_index_parts::shared_nodes;
use super::{LanguageEntry, StorageError, Store};

impl Store {
    /// One-time migration of an unindexed, immutable snapshot. Normal
    /// indexed planning never enters this method or reads stored fact objects.
    pub(super) fn bootstrap_legacy_index(
        &self,
        snapshot: &str,
        entry: &LanguageEntry,
    ) -> Result<String, StorageError> {
        let started = crate::perf::start();
        let mut objects = Vec::<Vec<FactRecord>>::new();
        let mut paths = Vec::new();
        let mut count = 0_u64;
        for file in entry.files.as_deref().unwrap_or_default() {
            let object = self.load_object(&file.object_id)?;
            count += 1;
            paths.push(file.path.clone());
            objects.push(object.records);
        }

        let groups = paths
            .into_iter()
            .zip(&objects)
            .map(|(path, records)| (path, records.iter().collect()))
            .collect::<BTreeMap<_, Vec<&FactRecord>>>();
        let shared = if entry.shared_object_id.is_empty() {
            Vec::new()
        } else {
            count += 1;
            self.load_object(&entry.shared_object_id)?.records
        };
        let shared_refs = shared.iter().collect::<Vec<_>>();
        let index_id = self.index_from_records(entry, &groups, &shared_nodes(&shared_refs))?;
        // The pointer names one exact historical snapshot and entry signature.
        // It is not a second mutable owner of the dependency graph.
        self.publish_bootstrap(snapshot, entry, &index_id)?;

        if let Some(started) = started {
            crate::perf::emit(
                "scan.dependency_bootstrap",
                started.elapsed(),
                &[
                    ("fact_object_reads", count),
                    ("index_written", 1),
                    ("source_files", objects.len() as u64),
                ],
            );
        }
        Ok(index_id)
    }
}

use std::collections::{BTreeMap, BTreeSet};

use crate::FactRecord;

use super::dependency_support::{collect_dependency_records, normalize_owner};
use super::{IncrementalScope, LanguageEntry, StorageError, Store};

struct DependencyData {
    objects: BTreeMap<String, Vec<FactRecord>>,
    node_owners: BTreeMap<String, String>,
    loaded_objects: u64,
}

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
        use super::dependency_index_read::IndexReader;

        let started = crate::perf::start();
        let (snapshot_id, manifest) = self.current()?;
        let entry = manifest.language(language).ok_or_else(|| {
            StorageError::Materialization(format!("snapshot has no {language} analysis"))
        })?;
        let index_id = if !entry.dependency_index_id.is_empty() {
            entry.dependency_index_id.clone()
        } else if let Some(index_id) = self.read_bootstrap(&snapshot_id, entry)? {
            index_id
        } else {
            // Legacy and Phase-2 incremental manifests have no embedded index.
            // Persist once for THIS immutable snapshot, never per query.
            let legacy = self.dependency_data(entry)?;
            let index_id =
                self.index_legacy_language(entry, &legacy.objects, &legacy.node_owners)?;
            self.publish_bootstrap(&snapshot_id, entry, &index_id)?;
            if let Some(started) = started {
                crate::perf::emit(
                    "scan.dependency_bootstrap",
                    started.elapsed(),
                    &[
                        ("fact_object_reads", legacy.loaded_objects),
                        ("index_written", 1),
                    ],
                );
            }
            index_id
        };
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

    fn dependency_data(&self, entry: &LanguageEntry) -> Result<DependencyData, StorageError> {
        let started = crate::perf::start();
        // Use the entry captured from CURRENT by the caller. A concurrent
        // publication must never change the bootstrap's source generation.

        let mut objects = BTreeMap::new();
        let mut node_owners = BTreeMap::new();
        let mut unresolved_candidates = BTreeSet::new();
        let known_paths: BTreeSet<String> = entry
            .files
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|file| file.path.clone())
            .collect();

        let mut fact_records = 0_u64;
        let mut loaded_objects = 0_u64;
        for file in entry.files.as_deref().unwrap_or_default() {
            let object = self.load_object(&file.object_id)?;
            fact_records += object.records.len() as u64;
            loaded_objects += 1;
            if crate::perf::enabled()
                && loaded_objects.is_multiple_of(512)
                && let Some(started) = started
            {
                crate::perf::emit(
                    "scan.dependency_rebuild_progress",
                    started.elapsed(),
                    &[
                        ("fact_object_reads", loaded_objects),
                        ("fact_records", fact_records),
                    ],
                );
            }
            collect_dependency_records(
                &object.records,
                &file.path,
                &mut node_owners,
                &mut unresolved_candidates,
            );
            objects.insert(file.path.clone(), object.records);
        }

        if !entry.shared_object_id.is_empty() {
            let shared = self.load_object(&entry.shared_object_id)?;
            loaded_objects += 1;
            fact_records += shared.records.len() as u64;
            for record in &shared.records {
                if let FactRecord::Node(node) = record {
                    let path = normalize_owner(&node.path);
                    if known_paths.contains(&path) {
                        node_owners.insert(node.id.clone(), path);
                    }
                }
            }
        }

        if let Some(started) = started {
            crate::perf::emit(
                "scan.dependency_rebuild",
                started.elapsed(),
                &[
                    ("fact_object_reads", loaded_objects),
                    ("fact_records", fact_records),
                    ("node_owners", node_owners.len() as u64),
                    ("unresolved_candidates", unresolved_candidates.len() as u64),
                    ("dependency_index_partitions", 0),
                ],
            );
        }
        Ok(DependencyData {
            objects,
            node_owners,
            loaded_objects,
        })
    }
}

use std::collections::{BTreeMap, BTreeSet};

use crate::FactRecord;

use super::dependency_support::{
    Graph, add_relation, collect_dependency_records, normalize_owner, one_hop_closure,
    python_module_candidate,
};
use super::{IncrementalScope, StorageError, Store};

struct DependencyData {
    objects: BTreeMap<String, Vec<FactRecord>>,
    node_owners: BTreeMap<String, String>,
    unresolved_candidates: BTreeSet<String>,
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
        let started = crate::perf::start();
        let data = self.dependency_data(language)?;
        let roots: BTreeSet<String> = roots.iter().cloned().collect();
        let mut found_roots = BTreeSet::new();
        let mut reverse = Graph::new();
        let mut forward = Graph::new();

        for (owner, records) in &data.objects {
            if roots.contains(owner) {
                found_roots.insert(owner.clone());
            }
            for record in records {
                let FactRecord::Edge(edge) = record else {
                    continue;
                };
                let Some(target_owner) = data.node_owners.get(&edge.target) else {
                    continue;
                };
                if target_owner == owner {
                    continue;
                }
                add_relation(&mut reverse, target_owner, owner);
                add_relation(&mut forward, owner, target_owner);
            }
        }

        let mut full_required = found_roots.len() != roots.len();
        if !additions.is_empty() {
            if language != "python" {
                full_required = true;
            } else {
                for path in additions {
                    let Some(candidate) = python_module_candidate(path) else {
                        full_required = true;
                        break;
                    };
                    if data.unresolved_candidates.contains(&candidate) {
                        full_required = true;
                        break;
                    }
                }
            }
        }

        let emit = one_hop_closure(&roots, &reverse);
        let emit_set = emit.iter().cloned().collect();
        let context = one_hop_closure(&emit_set, &forward);
        if let Some(started) = started {
            crate::perf::emit(
                "scan.dependency_scope",
                started.elapsed(),
                &[
                    ("roots", roots.len() as u64),
                    ("emit_files", emit.len() as u64),
                    ("context_files", context.len() as u64),
                    ("forward_owners", forward.len() as u64),
                    ("reverse_owners", reverse.len() as u64),
                    ("full_required", u64::from(full_required)),
                ],
            );
        }
        Ok(IncrementalScope {
            full_required,
            emit,
            context,
        })
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

    fn dependency_data(&self, language: &str) -> Result<DependencyData, StorageError> {
        let started = crate::perf::start();
        let (_, manifest) = self.current()?;
        let entry = manifest.language(language).cloned().ok_or_else(|| {
            StorageError::Materialization(format!("snapshot has no {language} analysis"))
        })?;

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
            unresolved_candidates,
        })
    }
}

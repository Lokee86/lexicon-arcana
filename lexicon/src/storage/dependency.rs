use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::FactRecord;

use super::{IncrementalScope, StorageError, Store};

type Graph = BTreeMap<String, BTreeSet<String>>;

struct DependencyData {
    objects: BTreeMap<String, Vec<FactRecord>>,
    node_owners: BTreeMap<String, String>,
    unresolved: BTreeSet<String>,
}

impl Store {
    pub fn incremental_scope(
        &self,
        language: &str,
        roots: &[String],
    ) -> Result<IncrementalScope, StorageError> {
        let data = self.dependency_data(language)?;
        let roots: BTreeSet<String> = roots.iter().cloned().collect();
        let mut found_roots = BTreeSet::new();
        let mut full_required = false;
        let mut reverse = Graph::new();
        let mut forward = Graph::new();

        for (owner, records) in &data.objects {
            let direct_root = roots.contains(owner);
            if direct_root {
                found_roots.insert(owner.clone());
            }
            for record in records {
                if direct_root {
                    if let FactRecord::Unresolved(value) = record
                        && repository_sensitive_unresolved(&value.reason)
                    {
                        full_required = true;
                    }
                    if let FactRecord::Edge(edge) = record
                        && semantic_relation(&edge.relation)
                        && data
                            .node_owners
                            .get(&edge.target)
                            .is_some_and(|target| target != owner)
                    {
                        full_required = true;
                    }
                }
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

        if found_roots.len() != roots.len() {
            full_required = true;
        }
        let mut emit_seeds: Vec<String> = roots.into_iter().collect();
        emit_seeds.extend(data.unresolved);
        let emit = closure(&emit_seeds, &reverse);
        let context = closure(&emit, &forward);
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
        let (_, manifest) = self.current()?;
        let entry = manifest.language(language).cloned().ok_or_else(|| {
            StorageError::Materialization(format!("snapshot has no {language} analysis"))
        })?;

        let mut objects = BTreeMap::new();
        let mut node_owners = BTreeMap::new();
        let mut unresolved = BTreeSet::new();
        for file in entry.files.as_deref().unwrap_or_default() {
            let object = self.load_object(&file.object_id)?;
            for record in &object.records {
                if let FactRecord::Node(node) = record {
                    node_owners.insert(node.id.clone(), file.path.clone());
                }
                if matches!(record, FactRecord::Unresolved(_)) {
                    unresolved.insert(file.path.clone());
                }
            }
            objects.insert(file.path.clone(), object.records);
        }
        Ok(DependencyData {
            objects,
            node_owners,
            unresolved,
        })
    }
}

fn repository_sensitive_unresolved(reason: &str) -> bool {
    matches!(
        reason,
        "missing-target" | "ambiguous-target" | "generated-target"
    )
}

fn semantic_relation(relation: &str) -> bool {
    !matches!(relation, "contains" | "defines")
}

fn add_relation(graph: &mut Graph, source: &str, target: &str) {
    graph
        .entry(source.to_owned())
        .or_default()
        .insert(target.to_owned());
}

fn closure(seeds: &[String], graph: &Graph) -> Vec<String> {
    let mut selected = BTreeSet::new();
    let mut queue: VecDeque<String> = seeds.iter().cloned().collect();
    while let Some(current) = queue.pop_front() {
        if current.is_empty() || !selected.insert(current.clone()) {
            continue;
        }
        if let Some(next) = graph.get(&current) {
            queue.extend(next.iter().cloned());
        }
    }
    selected.into_iter().collect()
}

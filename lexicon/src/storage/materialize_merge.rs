use std::collections::{BTreeMap, BTreeSet};

use crate::FactRecord;

use super::analysis::normalized_paths;
use super::materialize_support::materialization;
use super::{LanguageEntry, StorageError, Store};

impl Store {
    pub(crate) fn merge_language_shared_object(
        &self,
        entry: &LanguageEntry,
        previous: &LanguageEntry,
        updates: &[&FactRecord],
        owned_updates: &BTreeMap<String, Vec<&FactRecord>>,
        changed_files: &[String],
        removed_files: &[String],
    ) -> Result<String, StorageError> {
        let started = crate::perf::start();
        if previous.shared_object_id.is_empty() {
            let mut records = updates.iter().map(|record| (*record).clone()).collect();
            crate::facts::sort_records(&mut records)
                .map_err(|error| materialization(error.to_string()))?;
            let id = self.write_language_shared_records(entry, &records)?;
            if let Some(started) = started {
                crate::perf::emit(
                    "scan.shared_merge",
                    started.elapsed(),
                    &[
                        ("previous_shared_records", 0),
                        ("changed_file_objects_decoded", 0),
                        ("fact_object_reads", 0),
                    ],
                );
            }
            return Ok(id);
        }

        let shared = self.load_object(&previous.shared_object_id)?;
        let previous_shared_records = shared.records.len() as u64;
        let mut changed_file_objects = 0_u64;
        let mut invalidated: BTreeSet<String> =
            normalized_paths(changed_files).into_iter().collect();
        invalidated.extend(normalized_paths(removed_files));

        // A touched target is not automatically an invalid target. Scoped
        // analysis cannot regenerate edges originating in untouched modules;
        // discard those edges only when the touched target's exported identity
        // or semantic metadata actually changed.
        let replacement_nodes = owned_updates
            .values()
            .flat_map(|records| records.iter().copied())
            .chain(updates.iter().copied())
            .filter_map(|record| match record {
                FactRecord::Node(node) => Some((node.id.as_str(), node)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        let mut invalidated_nodes = BTreeSet::new();
        for record in &shared.records {
            if let FactRecord::Node(node) = record
                && record_touches_paths(record, &invalidated)
                && !replacement_nodes
                    .get(node.id.as_str())
                    .is_some_and(|new| stable_dependency_target(node, new))
            {
                invalidated_nodes.insert(node.id.clone());
            }
        }
        for file in previous.files.as_deref().unwrap_or_default() {
            if !invalidated.contains(&file.path) {
                continue;
            }
            let object = self.load_object(&file.object_id)?;
            changed_file_objects += 1;
            invalidated_nodes.extend(object.records.iter().filter_map(|record| {
                match record {
                    FactRecord::Node(node)
                        if !replacement_nodes
                            .get(node.id.as_str())
                            .is_some_and(|new| stable_dependency_target(node, new)) =>
                    {
                        Some(node.id.clone())
                    }
                    _ => None,
                }
            }));
        }

        let mut merged = BTreeMap::<String, FactRecord>::new();
        for record in shared.records {
            if !record_invalidated(&record, &invalidated, &invalidated_nodes) {
                merged.insert(record_key(&record)?, record);
            }
        }
        for record in updates {
            merged.insert(record_key(record)?, (*record).clone());
        }

        let mut records = merged.into_values().collect::<Vec<_>>();
        crate::facts::sort_records(&mut records)
            .map_err(|error| materialization(error.to_string()))?;
        let id = self.write_language_shared_records(entry, &records)?;
        if let Some(started) = started {
            crate::perf::emit(
                "scan.shared_merge",
                started.elapsed(),
                &[
                    ("previous_shared_records", previous_shared_records),
                    ("changed_file_objects_decoded", changed_file_objects),
                    ("fact_object_reads", 1 + changed_file_objects),
                ],
            );
        }
        Ok(id)
    }
}

fn stable_dependency_target(old: &crate::NodeRecord, new: &crate::NodeRecord) -> bool {
    // File byte hashes change for comment edits; a file node retains its
    // identity/relationships when every other exported field is unchanged.
    let mut old = old.clone();
    let mut new = new.clone();
    if old.kind == "file" && new.kind == "file" {
        old.content_id = None;
        new.content_id = None;
    }
    old == new
}

fn record_invalidated(
    record: &FactRecord,
    paths: &BTreeSet<String>,
    node_ids: &BTreeSet<String>,
) -> bool {
    if record_touches_paths(record, paths) {
        return true;
    }
    match record {
        FactRecord::Node(_) => false,
        FactRecord::Edge(edge) => {
            node_ids.contains(&edge.source) || node_ids.contains(&edge.target)
        }
        FactRecord::Unresolved(value) => node_ids.contains(&value.source),
    }
}

fn record_touches_paths(record: &FactRecord, paths: &BTreeSet<String>) -> bool {
    let mut candidates = Vec::with_capacity(3);
    if let Some(owner) = record.owner() {
        candidates.push(owner);
    }
    if let Some(span) = record.span() {
        candidates.push(span.path.as_str());
    }
    if let FactRecord::Node(node) = record {
        candidates.push(node.path.as_str());
    }
    candidates.into_iter().any(|path| {
        normalized_paths(&[path.to_owned()])
            .first()
            .is_some_and(|path| paths.contains(path))
    })
}

fn record_key(record: &FactRecord) -> Result<String, StorageError> {
    Ok(match record {
        FactRecord::Node(node) => format!(
            "0\\0{}\\0{}\\0{}\\0{}",
            node.id, node.kind, node.path, node.qualified_name,
        ),
        // A dependency can have multiple edges with the same endpoints,
        // relation and span but different source/category attributes.
        // Matching those edges by endpoints alone silently drops facts.
        FactRecord::Edge(edge) => format!(
            "1\\0{}",
            serde_json::to_string(&(
                &edge.source,
                &edge.target,
                &edge.relation,
                &edge.span,
                &edge.attributes,
                &edge.owner,
            ))?,
        ),
        // Resolvers may emit distinct unresolved candidates for an identical
        // expression. Retain each candidate and its attached provenance.
        FactRecord::Unresolved(value) => format!(
            "2\\0{}",
            serde_json::to_string(&(
                &value.source,
                &value.relation,
                &value.expression,
                &value.reason,
                &value.span,
                &value.candidate_name,
                &value.candidate_namespace,
                &value.attributes,
                &value.owner,
            ))?,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::record_key;
    use crate::{FactRecord, NodeRecord};

    #[test]
    fn node_merge_key_ignores_mutable_payload() {
        let mut left = NodeRecord {
            attributes: None,
            content_id: None,
            id: "id".into(),
            kind: "module".into(),
            name: "name".into(),
            owner: None,
            path: "a.py".into(),
            qualified_name: "a".into(),
            span: None,
        };
        let mut right = left.clone();
        right.name = "changed".into();
        left.attributes = Some(serde_json::json!({"old": true}));
        assert_eq!(
            record_key(&FactRecord::Node(left)).unwrap(),
            record_key(&FactRecord::Node(right)).unwrap()
        );
    }
}

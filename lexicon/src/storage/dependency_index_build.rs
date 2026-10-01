use std::collections::{BTreeMap, BTreeSet};

use crate::FactRecord;

use super::analysis::RecordGroups;
use super::dependency_index_model::{FileTopology, IndexRoot, VERSION, language_signature, shard};
use super::dependency_support::normalize_owner;
use super::{LanguageEntry, StorageError, Store};

impl Store {
    pub(super) fn index_full_language(
        &self,
        entry: &LanguageEntry,
        groups: &RecordGroups<'_>,
    ) -> Result<String, StorageError> {
        self.index_from_records(entry, &groups.owned, &groups.shared, None)
    }

    pub(super) fn index_legacy_language(
        &self,
        entry: &LanguageEntry,
        objects: &BTreeMap<String, Vec<FactRecord>>,
        owners: &BTreeMap<String, String>,
    ) -> Result<String, StorageError> {
        let references = objects
            .iter()
            .map(|(path, records)| (path.clone(), records.iter().collect()))
            .collect::<BTreeMap<_, Vec<&FactRecord>>>();
        self.index_from_records(entry, &references, &[], Some(owners))
    }

    fn index_from_records(
        &self,
        entry: &LanguageEntry,
        groups: &BTreeMap<String, Vec<&FactRecord>>,
        shared: &[&FactRecord],
        legacy_owners: Option<&BTreeMap<String, String>>,
    ) -> Result<String, StorageError> {
        let started = crate::perf::start();
        let files = entry.files.as_deref().unwrap_or_default();
        let known = files
            .iter()
            .map(|file| file.path.clone())
            .collect::<BTreeSet<_>>();
        let mut ownership = BTreeMap::new();
        let mut topology = known
            .iter()
            .map(|file| (file.clone(), FileTopology::default()))
            .collect::<BTreeMap<_, _>>();

        for file in files {
            if let Some(records) = groups.get(&file.path) {
                for record in records {
                    if let FactRecord::Node(node) = record {
                        ownership.insert(node.id.clone(), file.path.clone());
                    }
                }
            }
        }
        for record in shared {
            if let FactRecord::Node(node) = record {
                let path = normalize_owner(&node.path);
                if known.contains(&path) {
                    ownership.insert(node.id.clone(), path);
                }
            }
        }
        if let Some(legacy_owners) = legacy_owners {
            ownership = legacy_owners.clone();
        }

        let mut references = BTreeMap::<String, BTreeSet<String>>::new();
        let mut unresolved = BTreeMap::<String, BTreeSet<String>>::new();
        for (path, data) in &mut topology {
            let Some(records) = groups.get(path) else {
                continue;
            };
            for record in records {
                match record {
                    FactRecord::Node(node) => {
                        data.nodes.insert(node.id.clone());
                    }
                    FactRecord::Edge(edge) => {
                        data.referenced_nodes.insert(edge.target.clone());
                        references
                            .entry(edge.target.clone())
                            .or_default()
                            .insert(path.clone());
                        if let Some(target) = ownership.get(&edge.target)
                            && target != path
                        {
                            data.forward.insert(target.clone());
                        }
                    }
                    FactRecord::Unresolved(value)
                        if matches!(
                            value.reason.as_str(),
                            "missing-target"
                                | "ambiguous-target"
                                | "generated-target"
                                | "external-target"
                        ) && let Some(candidate) = value.candidate_name.as_deref()
                            && !candidate.trim().is_empty() =>
                    {
                        let candidate = candidate.trim().to_owned();
                        data.unresolved_candidates.insert(candidate.clone());
                        unresolved
                            .entry(candidate)
                            .or_default()
                            .insert(path.clone());
                    }
                    _ => {}
                }
            }
        }
        let edges = topology
            .iter()
            .flat_map(|(from, data)| {
                data.forward
                    .iter()
                    .map(move |to| (from.clone(), to.clone()))
            })
            .collect::<Vec<_>>();
        for (from, to) in &edges {
            if let Some(target) = topology.get_mut(to) {
                target.reverse.insert(from.clone());
            }
        }
        let file_count = topology.len();
        let root = IndexRoot {
            version: VERSION,
            language_signature: language_signature(entry)?,
            files: write_shards(self, topology)?,
            nodes: write_shards(self, ownership)?,
            references: write_shards(self, references)?,
            unresolved: write_shards(self, unresolved)?,
        };
        let index_id = self.write_index_object(&root)?;
        if let Some(started) = started {
            crate::perf::emit(
                "scan.dependency_index_build",
                started.elapsed(),
                &[
                    ("files", file_count as u64),
                    ("cross_file_edges", edges.len() as u64),
                    (
                        "partitions",
                        (root.files.len()
                            + root.nodes.len()
                            + root.references.len()
                            + root.unresolved.len()) as u64,
                    ),
                    ("fact_object_reads", 0),
                ],
            );
        }
        Ok(index_id)
    }
}

fn write_shards<T: serde::Serialize>(
    store: &Store,
    values: BTreeMap<String, T>,
) -> Result<BTreeMap<u8, String>, StorageError> {
    let mut partitions = BTreeMap::<u8, BTreeMap<String, T>>::new();
    for (key, value) in values {
        partitions
            .entry(shard(&key))
            .or_default()
            .insert(key, value);
    }
    partitions
        .into_iter()
        .map(|(partition, contents)| {
            store
                .write_index_object(&contents)
                .map(|id| (partition, id))
        })
        .collect()
}

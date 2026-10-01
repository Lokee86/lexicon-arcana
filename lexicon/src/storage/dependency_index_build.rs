use std::collections::{BTreeMap, BTreeSet};

use crate::FactRecord;

use super::analysis::RecordGroups;
use super::dependency_index_model::{FileTopology, IndexRoot, VERSION, language_signature, shard};
use super::dependency_index_parts::{file_records, shared_nodes};
use super::{LanguageEntry, StorageError, Store};

impl Store {
    pub(super) fn index_full_language(
        &self,
        entry: &LanguageEntry,
        groups: &RecordGroups<'_>,
    ) -> Result<String, StorageError> {
        self.index_from_records(entry, &groups.owned, &shared_nodes(&groups.shared), None)
    }

    pub(super) fn index_legacy_language(
        &self,
        entry: &LanguageEntry,
        objects: &BTreeMap<String, Vec<FactRecord>>,
        owners: &BTreeMap<String, String>,
        shared: &BTreeMap<String, String>,
    ) -> Result<String, StorageError> {
        let records = objects
            .iter()
            .map(|(path, data)| (path.clone(), data.iter().collect()))
            .collect::<BTreeMap<_, Vec<&FactRecord>>>();
        let mut paths = BTreeMap::<String, BTreeSet<String>>::new();
        for (id, path) in shared {
            paths.entry(path.clone()).or_default().insert(id.clone());
        }
        self.index_from_records(entry, &records, &paths, Some(owners))
    }

    fn index_from_records(
        &self,
        entry: &LanguageEntry,
        groups: &BTreeMap<String, Vec<&FactRecord>>,
        shared: &BTreeMap<String, BTreeSet<String>>,
        legacy_owners: Option<&BTreeMap<String, String>>,
    ) -> Result<String, StorageError> {
        let started = crate::perf::start();
        let known = entry
            .files
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|file| file.path.clone())
            .collect::<BTreeSet<_>>();
        let mut topology = BTreeMap::<String, FileTopology>::new();
        let mut ownership = BTreeMap::new();
        for path in &known {
            let data = groups
                .get(path)
                .map_or_else(FileTopology::default, |records| file_records(records));
            for id in &data.nodes {
                ownership.insert(id.clone(), path.clone());
            }
            topology.insert(path.clone(), data);
        }
        for (path, ids) in shared {
            if known.contains(path) {
                for id in ids {
                    ownership.insert(id.clone(), path.clone());
                }
            }
        }
        if let Some(legacy_owners) = legacy_owners {
            ownership = legacy_owners.clone();
        }
        let mut references = BTreeMap::<String, BTreeSet<String>>::new();
        let mut unresolved = BTreeMap::<String, BTreeSet<String>>::new();
        for (path, data) in &mut topology {
            for target in &data.referenced_nodes {
                references
                    .entry(target.clone())
                    .or_default()
                    .insert(path.clone());
                if let Some(owner) = ownership.get(target)
                    && owner != path
                {
                    data.forward.insert(owner.clone());
                }
            }
            for candidate in &data.unresolved_candidates {
                unresolved
                    .entry(candidate.clone())
                    .or_default()
                    .insert(path.clone());
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
            shared_paths: write_shards(self, shared.clone())?,
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
                            + root.unresolved.len()
                            + root.shared_paths.len()) as u64,
                    ),
                    ("fact_object_reads", 0),
                ],
            );
        }
        Ok(index_id)
    }
}

pub(super) fn write_shards<T: serde::Serialize>(
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
        .map(|(part, data)| store.write_index_object(&data).map(|id| (part, id)))
        .collect()
}

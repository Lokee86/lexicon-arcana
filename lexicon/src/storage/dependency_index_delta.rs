use std::collections::{BTreeMap, BTreeSet};

use crate::FactRecord;

use super::dependency_index_build::write_shards;
use super::dependency_index_delta_links::{refresh_links, update_lookup};
use super::dependency_index_model::{FileTopology, VERSION, language_signature};
use super::dependency_index_partition::Partitions;
use super::dependency_index_parts::file_records;
use super::dependency_support::normalize_owner;
use super::{LanguageEntry, StorageError, Store};

impl Store {
    pub(super) fn index_incremental_language(
        &self,
        previous: &LanguageEntry,
        next: &LanguageEntry,
        records: &BTreeMap<String, Vec<&FactRecord>>,
        changed: &BTreeSet<String>,
        removed: &BTreeSet<String>,
    ) -> Result<String, StorageError> {
        let started = crate::perf::start();
        let id = self.previous_index_id(previous)?;
        let mut root = self.validated_index(&id, previous)?;
        // Phase 2 roots predate the shared-path lookup. Read ONLY the old shared
        // object once when upgrading them; all later generations reuse shards.
        let v1_shared_reads = u64::from(root.version == 1 && !previous.shared_object_id.is_empty());
        if v1_shared_reads != 0 {
            let object = self.load_object(&previous.shared_object_id)?;
            let mut paths = BTreeMap::<String, BTreeSet<String>>::new();
            for record in &object.records {
                if let FactRecord::Node(node) = record {
                    paths
                        .entry(normalize_owner(&node.path))
                        .or_default()
                        .insert(node.id.clone());
                }
            }
            root.shared_paths = write_shards(self, paths)?;
        }

        let known = next
            .files
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|file| file.path.clone())
            .collect::<BTreeSet<_>>();
        let touched = changed.union(removed).cloned().collect::<BTreeSet<_>>();
        let mut files = Partitions::<FileTopology>::new(self, root.files);
        let mut owners = Partitions::<String>::new(self, root.nodes);
        let mut refs = Partitions::<BTreeSet<String>>::new(self, root.references);
        let mut unresolved = Partitions::<BTreeSet<String>>::new(self, root.unresolved);
        let mut shared = Partitions::<BTreeSet<String>>::new(self, root.shared_paths);

        let mut updated = BTreeMap::new();
        let mut changed_ids = BTreeSet::new();
        let mut desired_owners = BTreeMap::<String, String>::new();
        for path in &touched {
            let old = files.get(path)?;
            let next_file = if known.contains(path) {
                Some(file_records(
                    records.get(path).map(Vec::as_slice).unwrap_or(&[]),
                ))
            } else {
                None
            };
            if let Some(data) = &old {
                changed_ids.extend(data.nodes.iter().cloned());
            }
            if let Some(data) = &next_file {
                changed_ids.extend(data.nodes.iter().cloned());
                for id in &data.nodes {
                    assign(&mut desired_owners, id, path)?;
                }
            }
            // Shared-node path ownership overrides local node ownership.
            for id in shared.get(path)?.unwrap_or_default() {
                changed_ids.insert(id.clone());
                if known.contains(path) {
                    assign(&mut desired_owners, &id, path)?;
                }
            }
            update_lookup(
                &mut refs,
                path,
                &old.as_ref()
                    .map(|v| v.referenced_nodes.clone())
                    .unwrap_or_default(),
                &next_file
                    .as_ref()
                    .map(|v| v.referenced_nodes.clone())
                    .unwrap_or_default(),
            )?;
            update_lookup(
                &mut unresolved,
                path,
                &old.as_ref()
                    .map(|v| v.unresolved_candidates.clone())
                    .unwrap_or_default(),
                &next_file
                    .as_ref()
                    .map(|v| v.unresolved_candidates.clone())
                    .unwrap_or_default(),
            )?;
            updated.insert(path.clone(), next_file);
        }

        let mut affected = touched.clone();
        for id in &changed_ids {
            affected.extend(refs.get(id)?.unwrap_or_default());
            let old = owners.get(id)?;
            match desired_owners.get(id) {
                Some(path) => {
                    if old
                        .as_ref()
                        .is_some_and(|owner| !touched.contains(owner) && owner != path)
                    {
                        return Err(StorageError::UnsafeIndexDelta(format!(
                            "node {id} has conflicting retained ownership"
                        )));
                    }
                    owners.set(id, Some(path.clone()))?;
                }
                None if old.as_ref().is_some_and(|owner| touched.contains(owner)) => {
                    owners.set(id, None)?;
                }
                None => {}
            }
        }
        refresh_links(&mut files, &mut owners, &affected, &updated, &known)?;

        let (file_ids, file_reads, file_writes) = files.finish()?;
        let (owner_ids, owner_reads, owner_writes) = owners.finish()?;
        let (ref_ids, ref_reads, ref_writes) = refs.finish()?;
        let (unresolved_ids, unresolved_reads, unresolved_writes) = unresolved.finish()?;
        let (shared_ids, shared_reads, shared_writes) = shared.finish()?;
        root.version = VERSION;
        root.language_signature = language_signature(next)?;
        root.files = file_ids;
        root.nodes = owner_ids;
        root.references = ref_ids;
        root.unresolved = unresolved_ids;
        root.shared_paths = shared_ids;
        let id = self.write_index_object(&root)?;
        if let Some(started) = started {
            crate::perf::emit(
                "scan.dependency_index_delta",
                started.elapsed(),
                &[
                    ("changed_files", touched.len() as u64),
                    ("affected_files", affected.len() as u64),
                    (
                        "loaded_partitions",
                        file_reads + owner_reads + ref_reads + unresolved_reads + shared_reads,
                    ),
                    (
                        "rewritten_partitions",
                        file_writes + owner_writes + ref_writes + unresolved_writes + shared_writes,
                    ),
                    ("fact_object_reads", v1_shared_reads),
                ],
            );
        }
        Ok(id)
    }

    fn previous_index_id(&self, previous: &LanguageEntry) -> Result<String, StorageError> {
        if !previous.dependency_index_id.is_empty() {
            return Ok(previous.dependency_index_id.clone());
        }
        let (snapshot, manifest) = self.current()?;
        if manifest.language(&previous.language) != Some(previous) {
            return Err(StorageError::UnsafeIndexDelta(
                "previous language entry does not match CURRENT".into(),
            ));
        }
        if self.read_bootstrap(&snapshot, previous)?.is_none() {
            self.incremental_scope(&previous.language, &[])?;
        }
        self.read_bootstrap(&snapshot, previous)?.ok_or_else(|| {
            StorageError::UnsafeIndexDelta("legacy topology bootstrap missing".into())
        })
    }
}

fn assign(
    desired: &mut BTreeMap<String, String>,
    id: &str,
    owner: &str,
) -> Result<(), StorageError> {
    if desired
        .insert(id.to_owned(), owner.to_owned())
        .is_some_and(|previous| previous != owner)
    {
        return Err(StorageError::UnsafeIndexDelta(format!(
            "node {id} has multiple new owners"
        )));
    }
    Ok(())
}

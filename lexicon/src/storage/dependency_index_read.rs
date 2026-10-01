use std::collections::{BTreeMap, BTreeSet};

use super::dependency_index_model::{FileTopology, IndexRoot, shard};
use super::dependency_support::python_module_candidate;
use super::{IncrementalScope, LanguageEntry, StorageError, Store};

pub(super) struct IndexReader<'a> {
    store: &'a Store,
    root: IndexRoot,
    files: BTreeMap<u8, BTreeMap<String, FileTopology>>,
    candidates: BTreeMap<u8, BTreeMap<String, BTreeSet<String>>>,
    loaded: u64,
}

impl<'a> IndexReader<'a> {
    pub(super) fn open(
        store: &'a Store,
        entry: &LanguageEntry,
        index_id: &str,
    ) -> Result<Self, StorageError> {
        Ok(Self {
            store,
            root: store.validated_index(index_id, entry)?,
            files: BTreeMap::new(),
            candidates: BTreeMap::new(),
            loaded: 0,
        })
    }

    fn file(&mut self, path: &str) -> Result<Option<FileTopology>, StorageError> {
        let group = shard(path);
        if !self.files.contains_key(&group) {
            let values = match self.root.files.get(&group) {
                Some(id) => {
                    self.loaded += 1;
                    self.store.load_index_object(id)?
                }
                None => BTreeMap::new(),
            };
            self.files.insert(group, values);
        }
        Ok(self
            .files
            .get(&group)
            .and_then(|group| group.get(path))
            .cloned())
    }

    fn candidate_present(&mut self, name: &str) -> Result<bool, StorageError> {
        let group = shard(name);
        if !self.candidates.contains_key(&group) {
            let values = match self.root.unresolved.get(&group) {
                Some(id) => {
                    self.loaded += 1;
                    self.store.load_index_object(id)?
                }
                None => BTreeMap::new(),
            };
            self.candidates.insert(group, values);
        }
        Ok(self
            .candidates
            .get(&group)
            .is_some_and(|group| group.contains_key(name)))
    }

    pub(super) fn scope(
        &mut self,
        language: &str,
        roots: &[String],
        additions: &[String],
    ) -> Result<IncrementalScope, StorageError> {
        let started = crate::perf::start();
        let roots = roots.iter().cloned().collect::<BTreeSet<_>>();
        let mut emit = roots.clone();
        let mut missing = false;
        for root in &roots {
            match self.file(root)? {
                Some(entry) => emit.extend(entry.reverse),
                None => missing = true,
            }
        }
        let mut context = emit.clone();
        for file in &emit {
            if let Some(entry) = self.file(file)? {
                context.extend(entry.forward);
            }
        }
        let mut full_required = missing;
        if !additions.is_empty() {
            if language != "python" {
                full_required = true;
            } else {
                for path in additions {
                    let Some(candidate) = python_module_candidate(path) else {
                        full_required = true;
                        break;
                    };
                    if self.candidate_present(&candidate)? {
                        full_required = true;
                        break;
                    }
                }
            }
        }
        if let Some(started) = started {
            crate::perf::emit(
                "scan.dependency_index_query",
                started.elapsed(),
                &[
                    ("roots", roots.len() as u64),
                    ("emit_files", emit.len() as u64),
                    ("context_files", context.len() as u64),
                    ("loaded_partitions", self.loaded),
                    ("fact_object_reads", 0),
                    ("full_required", u64::from(full_required)),
                ],
            );
        }
        Ok(IncrementalScope {
            full_required,
            emit: emit.into_iter().collect(),
            context: context.into_iter().collect(),
        })
    }
}

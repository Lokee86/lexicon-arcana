use std::collections::{BTreeMap, BTreeSet};

use serde::{Serialize, de::DeserializeOwned};

use super::dependency_index_model::shard;
use super::{StorageError, Store};

/// Copy-on-write partition editor. Untouched immutable IDs are reused.
pub(super) struct Partitions<'a, V> {
    store: &'a Store,
    ids: BTreeMap<u8, String>,
    data: BTreeMap<u8, BTreeMap<String, V>>,
    dirty: BTreeSet<u8>,
    reads: u64,
}

impl<'a, V: Clone + PartialEq + Serialize + DeserializeOwned> Partitions<'a, V> {
    pub(super) fn new(store: &'a Store, ids: BTreeMap<u8, String>) -> Self {
        Self {
            store,
            ids,
            data: BTreeMap::new(),
            dirty: BTreeSet::new(),
            reads: 0,
        }
    }

    fn load(&mut self, part: u8) -> Result<&mut BTreeMap<String, V>, StorageError> {
        if !self.data.contains_key(&part) {
            let values = match self.ids.get(&part) {
                Some(id) => {
                    self.reads += 1;
                    self.store.load_index_object(id)?
                }
                None => BTreeMap::new(),
            };
            self.data.insert(part, values);
        }
        Ok(self.data.get_mut(&part).expect("partition inserted"))
    }

    pub(super) fn get(&mut self, key: &str) -> Result<Option<V>, StorageError> {
        Ok(self.load(shard(key))?.get(key).cloned())
    }

    pub(super) fn set(&mut self, key: &str, value: Option<V>) -> Result<(), StorageError> {
        let part = shard(key);
        let entries = self.load(part)?;
        if entries.get(key) == value.as_ref() {
            return Ok(());
        }
        match value {
            Some(value) => {
                entries.insert(key.to_owned(), value);
            }
            None => {
                entries.remove(key);
            }
        }
        self.dirty.insert(part);
        Ok(())
    }

    pub(super) fn finish(mut self) -> Result<(BTreeMap<u8, String>, u64, u64), StorageError> {
        let rewritten = self.dirty.len() as u64;
        for part in self.dirty {
            let entries = self.data.remove(&part).expect("dirty partition loaded");
            if entries.is_empty() {
                self.ids.remove(&part);
            } else {
                self.ids
                    .insert(part, self.store.write_index_object(&entries)?);
            }
        }
        Ok((self.ids, self.reads, rewritten))
    }
}

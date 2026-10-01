use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use super::{CompactStringTable, StoreFormatError, StringId};

const NO_COLLISION: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct TempStringId(pub u32);

#[derive(Clone, Copy)]
struct Entry {
    offset: u64,
    len: u32,
    next_collision: u32,
}

#[derive(Default)]
pub(crate) struct StagedStringArena {
    bytes: Vec<u8>,
    entries: Vec<Entry>,
    index: HashMap<u64, u32>,
}

impl StagedStringArena {
    pub(crate) fn intern(&mut self, value: &str) -> Result<TempStringId, StoreFormatError> {
        let hash = string_hash(value);
        let mut current = self.index.get(&hash).copied().unwrap_or(NO_COLLISION);
        while current != NO_COLLISION {
            let id = TempStringId(current);
            if self.value(id) == value {
                return Ok(id);
            }
            current = self.entries[current as usize].next_collision;
        }

        if self.entries.len() >= super::format::ABSENT_STRING_ID as usize {
            return Err(StoreFormatError::TooManyStrings);
        }
        let len = u32::try_from(value.len()).map_err(|_| StoreFormatError::StringTooLong)?;
        let offset = u64::try_from(self.bytes.len()).map_err(|_| StoreFormatError::SizeOverflow)?;
        self.bytes
            .len()
            .checked_add(value.len())
            .ok_or(StoreFormatError::SizeOverflow)?;

        let id = TempStringId(self.entries.len() as u32);
        let next_collision = self.index.insert(hash, id.0).unwrap_or(NO_COLLISION);
        self.bytes.extend_from_slice(value.as_bytes());
        self.entries.push(Entry {
            offset,
            len,
            next_collision,
        });
        Ok(id)
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn freeze(
        self,
        used: &[bool],
    ) -> Result<(CompactStringTable, Vec<StringId>), StoreFormatError> {
        assert_eq!(used.len(), self.entries.len(), "string liveness length");

        let Self {
            bytes,
            entries,
            index,
        } = self;
        drop(index);

        let mut live = used
            .iter()
            .enumerate()
            .filter_map(|(index, used)| used.then_some(TempStringId(index as u32)))
            .collect::<Vec<_>>();
        live.sort_unstable_by(|left, right| {
            value_from(&bytes, &entries, *left).cmp(value_from(&bytes, &entries, *right))
        });

        let mut remap = vec![StringId::ABSENT; entries.len()];
        for (new, old) in live.iter().copied().enumerate() {
            remap[old.0 as usize] = StringId(new as u32);
        }

        let values = live.iter().map(|id| value_from(&bytes, &entries, *id));
        let strings = CompactStringTable::from_sorted_refs(values)?;
        Ok((strings, remap))
    }

    fn value(&self, id: TempStringId) -> &str {
        value_from(&self.bytes, &self.entries, id)
    }

    #[cfg(test)]
    pub(crate) fn byte_len(&self) -> usize {
        self.bytes.len()
    }
}

fn value_from<'a>(bytes: &'a [u8], entries: &[Entry], id: TempStringId) -> &'a str {
    let entry = entries[id.0 as usize];
    let start = entry.offset as usize;
    let end = start + entry.len as usize;
    std::str::from_utf8(&bytes[start..end]).expect("interned string bytes are valid UTF-8")
}

fn string_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
#[path = "string_arena_tests.rs"]
mod tests;

use std::collections::BTreeSet;

use super::StoreFormatError;
use super::format::ABSENT_STRING_ID;
use crate::repository::{RepositoryFacts, UnresolvedReason};

#[path = "strings_binary.rs"]
mod binary;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct StringId(pub u32);

impl StringId {
    pub const ABSENT: Self = Self(ABSENT_STRING_ID);

    pub const fn optional(value: Option<Self>) -> Self {
        match value {
            Some(value) => value,
            None => Self::ABSENT,
        }
    }

    pub const fn present(self) -> Option<Self> {
        if self.0 == ABSENT_STRING_ID {
            None
        } else {
            Some(self)
        }
    }
}

pub(crate) trait StringIdLookup {
    fn id(&self, value: &str) -> Result<StringId, StoreFormatError>;
}

#[derive(Debug, Default)]
pub struct StringTableBuilder {
    strings: BTreeSet<String>,
}

impl StringTableBuilder {
    pub fn insert(&mut self, value: impl Into<String>) {
        self.strings.insert(value.into());
    }

    pub(crate) fn insert_ref(&mut self, value: &str) {
        if !self.strings.contains(value) {
            self.strings.insert(value.to_owned());
        }
    }

    pub fn collect_facts(&mut self, facts: &RepositoryFacts) {
        for node in &facts.nodes {
            self.insert_ref(&node.path);
            self.insert_ref(&node.name);
            self.insert_ref(&node.qualified_name);
            if let Some(span) = &node.span {
                self.insert_ref(&span.path);
            }
        }
        for edge in &facts.edges {
            if let Some(span) = &edge.span {
                self.insert_ref(&span.path);
            }
        }
        for reference in &facts.unresolved {
            self.insert_ref(&reference.expression);
            if let Some(value) = &reference.candidate_namespace {
                self.insert_ref(value);
            }
            if let Some(value) = &reference.candidate_name {
                self.insert_ref(value);
            }
            if let UnresolvedReason::Unknown(value) = &reference.reason {
                self.insert_ref(value);
            }
            if let Some(span) = &reference.span {
                self.insert_ref(&span.path);
            }
        }
    }

    pub fn finish(self) -> Result<CompactStringTable, StoreFormatError> {
        CompactStringTable::from_sorted(self.strings.into_iter().collect())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactStringTable {
    blob: Vec<u8>,
    offsets: Vec<u64>,
}

impl StringIdLookup for CompactStringTable {
    fn id(&self, value: &str) -> Result<StringId, StoreFormatError> {
        CompactStringTable::id(self, value)
    }
}

impl CompactStringTable {
    pub(crate) fn from_sorted(strings: Vec<String>) -> Result<Self, StoreFormatError> {
        Self::from_sorted_refs(strings.iter().map(String::as_str))
    }

    pub(crate) fn from_sorted_refs<'a>(
        values: impl ExactSizeIterator<Item = &'a str> + Clone,
    ) -> Result<Self, StoreFormatError> {
        let count = values.len();
        if count > ABSENT_STRING_ID as usize {
            return Err(StoreFormatError::TooManyStrings);
        }

        let blob_len = values.clone().try_fold(0usize, |size, value| {
            size.checked_add(value.len())
                .ok_or(StoreFormatError::SizeOverflow)
        })?;
        let mut blob = Vec::with_capacity(blob_len);
        let mut offsets = Vec::with_capacity(count + 1);
        offsets.push(0);

        for value in values {
            blob.extend_from_slice(value.as_bytes());
            offsets.push(u64::try_from(blob.len()).map_err(|_| StoreFormatError::SizeOverflow)?);
        }

        Ok(Self { blob, offsets })
    }

    pub fn len(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &str> {
        (0..self.len()).map(|index| {
            self.get(StringId(index as u32))
                .expect("compact string table contains valid offsets")
        })
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn id(&self, value: &str) -> Result<StringId, StoreFormatError> {
        let mut low = 0usize;
        let mut high = self.len();
        while low < high {
            let middle = low + (high - low) / 2;
            match self
                .get(StringId(middle as u32))
                .expect("compact string table contains valid offsets")
                .cmp(value)
            {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Ok(StringId(middle as u32)),
            }
        }
        Err(StoreFormatError::MissingString)
    }

    pub fn get(&self, id: StringId) -> Result<&str, StoreFormatError> {
        if id == StringId::ABSENT {
            return Err(StoreFormatError::AbsentString);
        }
        let index = id.0 as usize;
        let start = *self
            .offsets
            .get(index)
            .ok_or(StoreFormatError::InvalidStringId(id.0))?;
        let end = *self
            .offsets
            .get(index + 1)
            .ok_or(StoreFormatError::InvalidStringId(id.0))?;
        let start = usize::try_from(start).map_err(|_| StoreFormatError::SizeOverflow)?;
        let end = usize::try_from(end).map_err(|_| StoreFormatError::SizeOverflow)?;
        std::str::from_utf8(&self.blob[start..end]).map_err(|_| StoreFormatError::InvalidUtf8)
    }

    pub fn optional(&self, id: StringId) -> Result<Option<&str>, StoreFormatError> {
        id.present().map(|id| self.get(id)).transpose()
    }

    pub(super) fn parts(&self) -> (&[u8], &[u64]) {
        (&self.blob, &self.offsets)
    }

    pub(super) fn from_blob_parts(
        blob: Vec<u8>,
        offsets: Vec<u64>,
    ) -> Result<Self, StoreFormatError> {
        let count = offsets.len().saturating_sub(1);
        if count > ABSENT_STRING_ID as usize {
            return Err(StoreFormatError::TooManyStrings);
        }
        Ok(Self { blob, offsets })
    }
}

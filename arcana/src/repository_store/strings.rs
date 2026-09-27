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
        CompactStringTable::new(self.strings.into_iter().collect())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactStringTable {
    strings: Vec<String>,
}

impl StringIdLookup for CompactStringTable {
    fn id(&self, value: &str) -> Result<StringId, StoreFormatError> {
        CompactStringTable::id(self, value)
    }
}

impl CompactStringTable {
    fn new(strings: Vec<String>) -> Result<Self, StoreFormatError> {
        if strings.len() > ABSENT_STRING_ID as usize {
            return Err(StoreFormatError::TooManyStrings);
        }
        Ok(Self { strings })
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    pub fn id(&self, value: &str) -> Result<StringId, StoreFormatError> {
        let index = self
            .strings
            .binary_search_by(|candidate| candidate.as_str().cmp(value))
            .map_err(|_| StoreFormatError::MissingString)?;
        Ok(StringId(index as u32))
    }

    pub fn get(&self, id: StringId) -> Result<&str, StoreFormatError> {
        if id == StringId::ABSENT {
            return Err(StoreFormatError::AbsentString);
        }
        self.strings
            .get(id.0 as usize)
            .map(String::as_str)
            .ok_or(StoreFormatError::InvalidStringId(id.0))
    }

    pub fn optional(&self, id: StringId) -> Result<Option<&str>, StoreFormatError> {
        id.present().map(|id| self.get(id)).transpose()
    }
}

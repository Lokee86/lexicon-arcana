use std::collections::BTreeSet;

use crate::repository::{RepositoryFacts, UnresolvedReason};

use super::{StoreFormatError, StringId, StringIdLookup};

pub(crate) struct BorrowedStringTable<'a> {
    strings: Vec<&'a str>,
}

impl<'a> BorrowedStringTable<'a> {
    pub fn collect(
        facts: &'a RepositoryFacts,
        ownership_paths: impl Iterator<Item = &'a str>,
    ) -> Result<Self, StoreFormatError> {
        let mut strings = BTreeSet::new();
        for node in &facts.nodes {
            strings.insert(node.path.as_str());
            strings.insert(node.name.as_str());
            strings.insert(node.qualified_name.as_str());
            if let Some(span) = &node.span {
                strings.insert(span.path.as_str());
            }
        }
        for edge in &facts.edges {
            if let Some(span) = &edge.span {
                strings.insert(span.path.as_str());
            }
        }
        for reference in &facts.unresolved {
            strings.insert(reference.expression.as_str());
            if let Some(value) = &reference.candidate_namespace {
                strings.insert(value.as_str());
            }
            if let Some(value) = &reference.candidate_name {
                strings.insert(value.as_str());
            }
            if let UnresolvedReason::Unknown(value) = &reference.reason {
                strings.insert(value.as_str());
            }
            if let Some(span) = &reference.span {
                strings.insert(span.path.as_str());
            }
        }
        strings.extend(ownership_paths);
        if strings.len() > super::format::ABSENT_STRING_ID as usize {
            return Err(StoreFormatError::TooManyStrings);
        }
        Ok(Self {
            strings: strings.into_iter().collect(),
        })
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn values(&self) -> impl ExactSizeIterator<Item = &str> {
        self.strings.iter().copied()
    }
}

impl StringIdLookup for BorrowedStringTable<'_> {
    fn id(&self, value: &str) -> Result<StringId, StoreFormatError> {
        let index = self
            .strings
            .binary_search(&value)
            .map_err(|_| StoreFormatError::MissingString)?;
        Ok(StringId(index as u32))
    }
}

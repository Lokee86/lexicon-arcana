//! Reader for immutable Lexicon snapshot storage.

use std::collections::BTreeMap;
use std::fmt;
use std::io;

use crate::repository::{FactFileError, RepositoryFacts};

mod binary;
mod binary_v2;
mod binary_v2_reader;
mod format;
#[cfg(test)]
mod format_tests;
mod object;
mod records;
mod snapshot;

#[cfg(test)]
mod binary_tests;
#[cfg(test)]
mod tests;

pub use snapshot::{current, load};

const SNAPSHOT_VERSION: u64 = 1;
const OBJECT_VERSION: u64 = 1;
const FACT_SCHEMA_VERSION: u64 = 1;

/// One complete, immutable Lexicon analysis state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexiconSnapshot {
    id: String,
    facts: RepositoryFacts,
    files: BTreeMap<(String, String), String>,
    shared_objects: BTreeMap<String, Option<String>>,
    compatibility_warnings: Vec<String>,
}

impl LexiconSnapshot {
    /// Reads and verifies the snapshot named by `.lexicon/CURRENT`.
    pub fn current(root: impl AsRef<std::path::Path>) -> Result<Self, LexiconSnapshotError> {
        snapshot::current(root)
    }

    /// Reads and verifies one immutable snapshot by its content address.
    pub fn load(root: impl AsRef<std::path::Path>, id: &str) -> Result<Self, LexiconSnapshotError> {
        snapshot::load(root, id)
    }

    /// Returns the SHA-256 snapshot identity, including its `sha256:` prefix.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns all fact records materialized from the snapshot's objects.
    pub const fn facts(&self) -> &RepositoryFacts {
        &self.facts
    }

    /// Returns compatibility degradations accepted while reading the snapshot.
    pub fn compatibility_warnings(&self) -> &[String] {
        &self.compatibility_warnings
    }

    /// Reports whether any language-level shared fact object changed.
    pub fn shared_objects_changed(&self, previous: &Self) -> bool {
        self.shared_objects != previous.shared_objects
    }

    /// Compares file object identities against an earlier snapshot.
    pub fn changed_paths(&self, previous: &Self) -> LexiconPathChanges {
        let mut added = Vec::new();
        let mut changed = Vec::new();
        let mut removed = Vec::new();

        for ((language, path), object_id) in &self.files {
            match previous.files.get(&(language.clone(), path.clone())) {
                None => added.push(path.clone()),
                Some(previous_id) if previous_id != object_id => changed.push(path.clone()),
                Some(_) => {}
            }
        }
        for (language, path) in previous.files.keys() {
            if !self.files.contains_key(&(language.clone(), path.clone())) {
                removed.push(path.clone());
            }
        }
        added.sort_unstable();
        added.dedup();
        changed.sort_unstable();
        changed.dedup();
        removed.sort_unstable();
        removed.dedup();
        LexiconPathChanges {
            added,
            changed,
            removed,
        }
    }
}

/// File paths whose content-addressed fact objects differ between snapshots.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LexiconPathChanges {
    pub added: Vec<String>,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
}

/// An error while reading or validating a Lexicon snapshot.
#[derive(Debug)]
pub enum LexiconSnapshotError {
    Io(io::Error),
    Json(serde_json::Error),
    Binary(String),
    Facts(FactFileError),
    InvalidCurrent,
    InvalidId(String),
    InvalidPath {
        field: &'static str,
        path: String,
    },
    Malformed(&'static str),
    UnsupportedSnapshotVersion(u64),
    UnsupportedObjectVersion(u64),
    UnsupportedSchemaVersion(u64),
    ContentHashMismatch {
        kind: &'static str,
        expected: String,
        actual: String,
    },
    MetadataMismatch(&'static str),
    ConflictingNode(String),
}

impl fmt::Display for LexiconSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Json(error) => write!(formatter, "Lexicon snapshot JSON is invalid: {error}"),
            Self::Binary(error) => write!(formatter, "Lexicon binary object is invalid: {error}"),
            Self::Facts(error) => error.fmt(formatter),
            Self::InvalidCurrent => formatter.write_str("Lexicon CURRENT is invalid"),
            Self::InvalidId(id) => write!(formatter, "invalid Lexicon snapshot/object ID {id:?}"),
            Self::InvalidPath { field, path } => {
                write!(formatter, "Lexicon {field} path is invalid: {path:?}")
            }
            Self::Malformed(reason) => write!(formatter, "Lexicon snapshot is malformed: {reason}"),
            Self::UnsupportedSnapshotVersion(version) => {
                write!(formatter, "unsupported Lexicon snapshot version {version}")
            }
            Self::UnsupportedObjectVersion(version) => {
                write!(formatter, "unsupported Lexicon object version {version}")
            }
            Self::UnsupportedSchemaVersion(version) => {
                write!(
                    formatter,
                    "unsupported Lexicon fact schema version {version}"
                )
            }
            Self::ContentHashMismatch {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "Lexicon {kind} hash mismatch: expected {expected}, got {actual}"
            ),
            Self::MetadataMismatch(field) => {
                write!(formatter, "Lexicon snapshot metadata mismatch in {field}")
            }
            Self::ConflictingNode(id) => {
                write!(formatter, "conflicting Lexicon node definition for {id}")
            }
        }
    }
}

impl std::error::Error for LexiconSnapshotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Facts(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for LexiconSnapshotError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for LexiconSnapshotError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<FactFileError> for LexiconSnapshotError {
    fn from(error: FactFileError) -> Self {
        Self::Facts(error)
    }
}

//! Reader for immutable Lexicon snapshot storage.

use std::fmt;
use std::io;

use crate::repository::{FactFileError, RepositoryFacts};
use crate::repository_store::{RepositoryStoreReadError, RepositoryStoreWriteError};

mod binary;
mod binary_v2;
mod binary_v2_reader;
#[allow(dead_code)]
mod binary_v2_stream;
mod format;
#[cfg(test)]
mod format_tests;
mod identity;
mod metadata;
mod object;
mod records;
#[cfg(test)]
mod records_tests;
mod snapshot;
mod snapshot_compact;
mod snapshot_compact_visit;
mod snapshot_support;
#[allow(dead_code)]
mod stream_compact;
#[allow(dead_code)]
mod stream_compact_convert;
#[allow(dead_code)]
mod stream_compact_legacy;
#[allow(dead_code)]
mod stream_compact_node;
mod stream_records;

#[cfg(test)]
mod binary_tests;
#[cfg(test)]
mod snapshot_compact_bench;
#[cfg(test)]
mod snapshot_compact_boundary_tests;
#[cfg(test)]
mod snapshot_compact_tests;
#[cfg(test)]
mod tests;

pub use metadata::{LexiconPathChanges, LexiconSnapshotMetadata};
pub use snapshot::{current, current_metadata, load, load_metadata};
#[doc(hidden)]
pub use snapshot_compact::{
    CompactLexiconDelta, CompactLexiconSnapshot, load_compact, load_compact_delta,
};

const SNAPSHOT_VERSION: u64 = 1;
const OBJECT_VERSION: u64 = 1;
const FACT_SCHEMA_VERSION: u64 = 1;

/// One complete, immutable Lexicon analysis state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexiconSnapshot {
    metadata: LexiconSnapshotMetadata,
    facts: RepositoryFacts,
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

    /// Returns the verified snapshot manifest metadata used by this state.
    pub const fn metadata(&self) -> &LexiconSnapshotMetadata {
        &self.metadata
    }

    /// Returns the SHA-256 snapshot identity, including its `sha256:` prefix.
    pub fn id(&self) -> &str {
        self.metadata.id()
    }

    /// Returns all fact records materialized from the snapshot's objects.
    pub const fn facts(&self) -> &RepositoryFacts {
        &self.facts
    }

    /// Transfers ownership of the materialized facts without cloning them.
    pub fn into_facts(self) -> RepositoryFacts {
        self.facts
    }

    /// Returns compatibility degradations accepted while reading the snapshot.
    pub fn compatibility_warnings(&self) -> &[String] {
        &self.compatibility_warnings
    }

    /// Reports whether any language-level shared fact object changed.
    pub fn shared_objects_changed(&self, previous: &Self) -> bool {
        self.metadata.shared_objects_changed(&previous.metadata)
    }

    /// Compares file object identities against an earlier snapshot.
    pub fn changed_paths(&self, previous: &Self) -> LexiconPathChanges {
        self.metadata.changed_paths(&previous.metadata)
    }
}

/// An error while reading or validating a Lexicon snapshot.
#[derive(Debug)]
pub enum LexiconSnapshotError {
    Io(io::Error),
    Json(serde_json::Error),
    Binary(String),
    Facts(FactFileError),
    RepositoryStore(RepositoryStoreWriteError),
    RepositoryStoreRead(RepositoryStoreReadError),
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
            Self::RepositoryStore(error) => error.fmt(formatter),
            Self::RepositoryStoreRead(error) => error.fmt(formatter),
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
            Self::RepositoryStore(error) => Some(error),
            Self::RepositoryStoreRead(error) => Some(error),
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

impl From<RepositoryStoreWriteError> for LexiconSnapshotError {
    fn from(error: RepositoryStoreWriteError) -> Self {
        Self::RepositoryStore(error)
    }
}

impl From<RepositoryStoreReadError> for LexiconSnapshotError {
    fn from(error: RepositoryStoreReadError) -> Self {
        Self::RepositoryStoreRead(error)
    }
}

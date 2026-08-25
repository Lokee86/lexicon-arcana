use std::fmt;
use std::io;

use crate::storage::{PackedError, QueryError};

use super::{ManifestError, OverlayError};

#[derive(Debug)]
pub enum SnapshotError {
    Io(io::Error),
    Manifest(ManifestError),
    Overlay(OverlayError),
    Packed(PackedError),
    Query(QueryError),
    InvalidRelativePath {
        field: &'static str,
        path: String,
    },
    ManifestMismatch {
        field: &'static str,
        expected: u64,
        actual: u64,
    },
    CompactionMismatch {
        field: &'static str,
        expected: u64,
        actual: u64,
    },
    SizeOverflow,
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Manifest(error) => error.fmt(formatter),
            Self::Overlay(error) => error.fmt(formatter),
            Self::Packed(error) => error.fmt(formatter),
            Self::Query(error) => error.fmt(formatter),
            Self::InvalidRelativePath { field, path } => write!(
                formatter,
                "snapshot {field} path '{path}' must be relative without parent traversal"
            ),
            Self::ManifestMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "snapshot manifest {field} mismatch: expected {expected:#x}, got {actual:#x}"
            ),
            Self::CompactionMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "compacted snapshot {field} mismatch: expected {expected:#x}, got {actual:#x}"
            ),
            Self::SizeOverflow => formatter.write_str("snapshot is too large to materialize"),
        }
    }
}

impl std::error::Error for SnapshotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Manifest(error) => Some(error),
            Self::Overlay(error) => Some(error),
            Self::Packed(error) => Some(error),
            Self::Query(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ManifestError> for SnapshotError {
    fn from(error: ManifestError) -> Self {
        Self::Manifest(error)
    }
}

impl From<OverlayError> for SnapshotError {
    fn from(error: OverlayError) -> Self {
        Self::Overlay(error)
    }
}

impl From<PackedError> for SnapshotError {
    fn from(error: PackedError) -> Self {
        Self::Packed(error)
    }
}

impl From<QueryError> for SnapshotError {
    fn from(error: QueryError) -> Self {
        Self::Query(error)
    }
}

impl From<io::Error> for SnapshotError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

use std::fmt;
use std::io;

use crate::repository_store::RepositoryStoreReadError;
use crate::snapshot::SnapshotError;

use super::{CatalogueError, FactFileError, RepositoryCompileError};

#[derive(Debug)]
pub enum RepositorySnapshotError {
    Io(io::Error),
    Graph(SnapshotError),
    Catalogue(CatalogueError),
    Facts(FactFileError),
    Compile(RepositoryCompileError),
    MalformedManifest(&'static str),
    UnsupportedManifestVersion(u64),
    UnsupportedRepositoryStoreVersion(u16),
    RepositoryStore(RepositoryStoreReadError),
    InvalidTextField(&'static str),
    InvalidComponentPath(&'static str),
    ArtifactMismatch {
        field: &'static str,
        expected: u64,
        actual: u64,
    },
    RepositoryIdentityMismatch {
        expected: u64,
        actual: u64,
    },
    InvalidUnresolvedArtifact,
}

impl fmt::Display for RepositorySnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Graph(error) => error.fmt(formatter),
            Self::Catalogue(error) => error.fmt(formatter),
            Self::Facts(error) => error.fmt(formatter),
            Self::Compile(error) => error.fmt(formatter),
            Self::MalformedManifest(reason) => {
                write!(formatter, "repository manifest is malformed: {reason}")
            }
            Self::UnsupportedManifestVersion(version) => write!(
                formatter,
                "repository manifest version {version} is unsupported"
            ),
            Self::UnsupportedRepositoryStoreVersion(version) => write!(
                formatter,
                "repository store version {version} is unsupported"
            ),
            Self::RepositoryStore(error) => error.fmt(formatter),
            Self::InvalidTextField(field) => {
                write!(formatter, "repository manifest field '{field}' is invalid")
            }
            Self::InvalidComponentPath(field) => {
                write!(formatter, "repository manifest path '{field}' is invalid")
            }
            Self::ArtifactMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "repository snapshot {field} mismatch: expected {expected:#x}, got {actual:#x}"
            ),
            Self::RepositoryIdentityMismatch { expected, actual } => write!(
                formatter,
                "repository identity mismatch: expected {expected:#x}, got {actual:#x}"
            ),
            Self::InvalidUnresolvedArtifact => {
                formatter.write_str("unresolved artifact contains node or edge facts")
            }
        }
    }
}

impl std::error::Error for RepositorySnapshotError {}

macro_rules! conversion {
    ($variant:ident, $source:ty) => {
        impl From<$source> for RepositorySnapshotError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}
conversion!(Io, io::Error);
conversion!(Graph, SnapshotError);
conversion!(Catalogue, CatalogueError);
conversion!(Facts, FactFileError);
conversion!(Compile, RepositoryCompileError);
conversion!(RepositoryStore, RepositoryStoreReadError);

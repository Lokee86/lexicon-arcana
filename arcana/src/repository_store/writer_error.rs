use std::fmt;
use std::io;

use crate::repository::{FactOwnershipError, NodeKey};

use super::StoreFormatError;
use super::format::FormatError;

#[derive(Debug)]
pub enum RepositoryStoreWriteError {
    Io(io::Error),
    Format(FormatError),
    Store(StoreFormatError),
    Ownership(FactOwnershipError),
    DuplicateConflictingNode { key: NodeKey },
    TooManyNodeOccurrences { key: NodeKey },
    MissingEdgeEndpoint { key: NodeKey },
    MissingUnresolvedSource { key: NodeKey },
    DuplicateCompactNodeOwner { key: NodeKey },
    TooManyNodes,
    TooManyContributions,
    SizeOverflow,
}

impl fmt::Display for RepositoryStoreWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Format(error) => error.fmt(formatter),
            Self::Store(error) => error.fmt(formatter),
            Self::Ownership(error) => error.fmt(formatter),
            Self::DuplicateConflictingNode { key } => {
                write!(formatter, "node key {key:?} has conflicting facts")
            }
            Self::TooManyNodeOccurrences { key } => {
                write!(
                    formatter,
                    "node key {key:?} has more than u32::MAX occurrences"
                )
            }
            Self::MissingEdgeEndpoint { key } => {
                write!(formatter, "edge references missing node key {key:?}")
            }
            Self::MissingUnresolvedSource { key } => {
                write!(
                    formatter,
                    "unresolved reference has missing source node key {key:?}"
                )
            }
            Self::DuplicateCompactNodeOwner { key } => {
                write!(formatter, "node key {key:?} has conflicting compact owners")
            }
            Self::TooManyNodes => formatter.write_str("repository has more than u32::MAX nodes"),
            Self::TooManyContributions => {
                formatter.write_str("repository ownership contribution count exceeds u64")
            }
            Self::SizeOverflow => formatter.write_str("repository.arcana size overflow"),
        }
    }
}

impl std::error::Error for RepositoryStoreWriteError {}

impl From<io::Error> for RepositoryStoreWriteError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<FormatError> for RepositoryStoreWriteError {
    fn from(error: FormatError) -> Self {
        Self::Format(error)
    }
}

impl From<StoreFormatError> for RepositoryStoreWriteError {
    fn from(error: StoreFormatError) -> Self {
        Self::Store(error)
    }
}

impl From<FactOwnershipError> for RepositoryStoreWriteError {
    fn from(error: FactOwnershipError) -> Self {
        Self::Ownership(error)
    }
}

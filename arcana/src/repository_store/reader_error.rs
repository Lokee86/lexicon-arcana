use std::fmt;
use std::io;

use crate::repository::RepositoryPathError;

use super::StoreFormatError;
use super::format::{FormatError, SectionKind};

#[derive(Debug)]
pub enum RepositoryStoreReadError {
    Io(io::Error),
    Format(FormatError),
    Store(StoreFormatError),
    FileLength { expected: u64, actual: u64 },
    PayloadChecksum,
    SectionChecksum(SectionKind),
    NonZeroPadding,
    InvalidNodeId(u32),
    InvalidPath(RepositoryPathError),
    InvalidOwnership,
}

impl fmt::Display for RepositoryStoreReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Format(error) => error.fmt(formatter),
            Self::Store(error) => error.fmt(formatter),
            Self::FileLength { expected, actual } => {
                write!(
                    formatter,
                    "repository.arcana length is {actual}, expected {expected}"
                )
            }
            Self::PayloadChecksum => {
                formatter.write_str("repository.arcana payload checksum mismatch")
            }
            Self::SectionChecksum(kind) => {
                write!(formatter, "repository.arcana {kind:?} checksum mismatch")
            }
            Self::NonZeroPadding => {
                formatter.write_str("repository.arcana alignment padding is non-zero")
            }
            Self::InvalidNodeId(id) => {
                write!(formatter, "repository.arcana contains invalid node id {id}")
            }
            Self::InvalidPath(error) => error.fmt(formatter),
            Self::InvalidOwnership => {
                formatter.write_str("repository.arcana ownership section is invalid")
            }
        }
    }
}

impl std::error::Error for RepositoryStoreReadError {}

impl From<io::Error> for RepositoryStoreReadError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<FormatError> for RepositoryStoreReadError {
    fn from(error: FormatError) -> Self {
        Self::Format(error)
    }
}

impl From<StoreFormatError> for RepositoryStoreReadError {
    fn from(error: StoreFormatError) -> Self {
        Self::Store(error)
    }
}

impl From<RepositoryPathError> for RepositoryStoreReadError {
    fn from(error: RepositoryPathError) -> Self {
        Self::InvalidPath(error)
    }
}

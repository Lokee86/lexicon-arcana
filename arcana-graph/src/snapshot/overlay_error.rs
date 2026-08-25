use std::fmt;
use std::io;

use crate::Edge;
use crate::storage::{DatasetError, PackedError};

#[derive(Debug)]
pub enum OverlayError {
    Io(io::Error),
    Packed(PackedError),
    Dataset(DatasetError),
    FileTooShort { actual: u64, minimum: u64 },
    InvalidMagic,
    UnsupportedVersion { found: u16 },
    InvalidHeaderLength { found: u16 },
    UnsupportedFlags { found: u32 },
    ReservedFieldSet { found: u32 },
    SizeOverflow,
    FileLengthMismatch { declared: u64, actual: u64 },
    LayoutMismatch { section: &'static str },
    PayloadChecksumMismatch { expected: u64, actual: u64 },
    OverlayChecksumMismatch { expected: u64, actual: u64 },
    BaseNodeCountMismatch { expected: u32, actual: u32 },
    BaseEdgeCountMismatch { expected: u64, actual: u64 },
    BaseChecksumMismatch { expected: u64, actual: u64 },
    VisibleEdgeCountMismatch { expected: u64, actual: u64 },
    VisibleChecksumMismatch { expected: u64, actual: u64 },
    UnsortedOperations { section: &'static str },
    OperationConflict { edge: Edge },
    RemovedEdgeMissing { edge: Edge },
    AddedEdgeExists { edge: Edge },
}

impl fmt::Display for OverlayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Packed(error) => error.fmt(formatter),
            Self::Dataset(error) => error.fmt(formatter),
            Self::FileTooShort { actual, minimum } => write!(
                formatter,
                "overlay file has {actual} bytes; at least {minimum} are required"
            ),
            Self::InvalidMagic => formatter.write_str("overlay file magic is invalid"),
            Self::UnsupportedVersion { found } => {
                write!(formatter, "overlay format version {found} is unsupported")
            }
            Self::InvalidHeaderLength { found } => {
                write!(formatter, "overlay header length {found} is invalid")
            }
            Self::UnsupportedFlags { found } => {
                write!(formatter, "overlay header flags {found:#x} are unsupported")
            }
            Self::ReservedFieldSet { found } => {
                write!(formatter, "overlay reserved field is nonzero: {found:#x}")
            }
            Self::SizeOverflow => formatter.write_str("overlay size exceeds supported limits"),
            Self::FileLengthMismatch { declared, actual } => write!(
                formatter,
                "overlay declares {declared} bytes but contains {actual}"
            ),
            Self::LayoutMismatch { section } => {
                write!(formatter, "overlay {section} section offset is invalid")
            }
            Self::PayloadChecksumMismatch { expected, actual } => write!(
                formatter,
                "overlay payload checksum mismatch: expected {expected:#x}, got {actual:#x}"
            ),
            Self::OverlayChecksumMismatch { expected, actual } => write!(
                formatter,
                "overlay operation checksum mismatch: expected {expected:#x}, got {actual:#x}"
            ),
            Self::BaseNodeCountMismatch { expected, actual } => write!(
                formatter,
                "overlay expects {expected} base nodes but graph has {actual}"
            ),
            Self::BaseEdgeCountMismatch { expected, actual } => write!(
                formatter,
                "overlay expects {expected} base edges but graph has {actual}"
            ),
            Self::BaseChecksumMismatch { expected, actual } => write!(
                formatter,
                "overlay expects base checksum {expected:#x} but graph has {actual:#x}"
            ),
            Self::VisibleEdgeCountMismatch { expected, actual } => write!(
                formatter,
                "overlay declares {expected} visible edges but produces {actual}"
            ),
            Self::VisibleChecksumMismatch { expected, actual } => write!(
                formatter,
                "overlay visible checksum mismatch: expected {expected:#x}, got {actual:#x}"
            ),
            Self::UnsortedOperations { section } => {
                write!(formatter, "overlay {section} operations are not canonical")
            }
            Self::OperationConflict { edge } => write!(
                formatter,
                "overlay both adds and removes edge {} -> {} of kind {}",
                edge.source.0, edge.target.0, edge.kind.0
            ),
            Self::RemovedEdgeMissing { edge } => write!(
                formatter,
                "overlay removes missing edge {} -> {} of kind {}",
                edge.source.0, edge.target.0, edge.kind.0
            ),
            Self::AddedEdgeExists { edge } => write!(
                formatter,
                "overlay adds existing edge {} -> {} of kind {}",
                edge.source.0, edge.target.0, edge.kind.0
            ),
        }
    }
}

impl std::error::Error for OverlayError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Packed(error) => Some(error),
            Self::Dataset(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for OverlayError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<PackedError> for OverlayError {
    fn from(error: PackedError) -> Self {
        Self::Packed(error)
    }
}

impl From<DatasetError> for OverlayError {
    fn from(error: DatasetError) -> Self {
        Self::Dataset(error)
    }
}

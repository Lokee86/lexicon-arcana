use std::fmt;
use std::io;

use crate::{Edge, NodeId};

/// Direction of an adjacency section or query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Forward,
    Reverse,
}

/// A logical graph dataset that cannot be packed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatasetError {
    EndpointOutOfRange { edge: Edge, node_count: u32 },
    DuplicateEdge { edge: Edge },
}

impl fmt::Display for DatasetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EndpointOutOfRange { edge, node_count } => write!(
                formatter,
                "edge {} -> {} references outside {} nodes",
                edge.source.0, edge.target.0, node_count
            ),
            Self::DuplicateEdge { edge } => write!(
                formatter,
                "duplicate edge {} -> {} of kind {}",
                edge.source.0, edge.target.0, edge.kind.0
            ),
        }
    }
}

impl std::error::Error for DatasetError {}

/// A node query that cannot be answered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryError {
    InvalidNode { node: NodeId, node_count: u32 },
}

impl fmt::Display for QueryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNode { node, node_count } => {
                write!(formatter, "node {} is outside {} nodes", node.0, node_count)
            }
        }
    }
}

impl std::error::Error for QueryError {}

/// A packed graph that cannot be written or opened.
#[derive(Debug)]
pub enum PackedError {
    Io(io::Error),
    Dataset(DatasetError),
    FileTooShort {
        actual: u64,
        minimum: u64,
    },
    InvalidMagic,
    UnsupportedVersion {
        found: u16,
    },
    InvalidHeaderLength {
        found: u16,
    },
    UnsupportedFlags {
        found: u32,
    },
    InvalidEndianMarker {
        found: u64,
    },
    SizeOverflow,
    FileLengthMismatch {
        declared: u64,
        actual: u64,
    },
    LayoutMismatch {
        section: &'static str,
    },
    PayloadChecksumMismatch {
        expected: u64,
        actual: u64,
    },
    DatasetChecksumMismatch {
        expected: u64,
        actual: u64,
    },
    InvalidOffsetTable {
        direction: Direction,
    },
    InvalidNeighbor {
        direction: Direction,
        node: NodeId,
        neighbor: NodeId,
        node_count: u32,
    },
    UnsortedAdjacency {
        direction: Direction,
        node: NodeId,
    },
}

impl fmt::Display for PackedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Dataset(error) => error.fmt(formatter),
            Self::FileTooShort { actual, minimum } => {
                write!(
                    formatter,
                    "packed file has {actual} bytes; at least {minimum} are required"
                )
            }
            Self::InvalidMagic => formatter.write_str("packed file magic is invalid"),
            Self::UnsupportedVersion { found } => {
                write!(formatter, "packed format version {found} is unsupported")
            }
            Self::InvalidHeaderLength { found } => {
                write!(formatter, "packed header length {found} is invalid")
            }
            Self::UnsupportedFlags { found } => {
                write!(formatter, "packed header flags {found:#x} are unsupported")
            }
            Self::InvalidEndianMarker { found } => {
                write!(formatter, "packed endian marker {found:#x} is invalid")
            }
            Self::SizeOverflow => formatter.write_str("packed graph size exceeds supported limits"),
            Self::FileLengthMismatch { declared, actual } => write!(
                formatter,
                "packed file declares {declared} bytes but contains {actual}"
            ),
            Self::LayoutMismatch { section } => {
                write!(formatter, "packed {section} section offset is invalid")
            }
            Self::PayloadChecksumMismatch { expected, actual } => write!(
                formatter,
                "packed payload checksum mismatch: expected {expected:#x}, got {actual:#x}"
            ),
            Self::DatasetChecksumMismatch { expected, actual } => write!(
                formatter,
                "packed dataset checksum mismatch: expected {expected:#x}, got {actual:#x}"
            ),
            Self::InvalidOffsetTable { direction } => {
                write!(formatter, "{direction:?} offset table is invalid")
            }
            Self::InvalidNeighbor {
                direction,
                node,
                neighbor,
                node_count,
            } => write!(
                formatter,
                "{direction:?} adjacency for node {} references node {} outside {} nodes",
                node.0, neighbor.0, node_count
            ),
            Self::UnsortedAdjacency { direction, node } => write!(
                formatter,
                "{direction:?} adjacency for node {} is unsorted or duplicated",
                node.0
            ),
        }
    }
}

impl std::error::Error for PackedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Dataset(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for PackedError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<DatasetError> for PackedError {
    fn from(error: DatasetError) -> Self {
        Self::Dataset(error)
    }
}

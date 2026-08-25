//! Packed immutable graph storage and correctness reference structures.

mod dataset;
mod error;
mod format;
mod oracle;
mod reader;
mod writer;

#[cfg(test)]
mod corruption_tests;
#[cfg(test)]
mod tests;

pub use error::{DatasetError, Direction, PackedError, QueryError};
pub use oracle::InMemoryGraph;
pub use reader::{PackedGraph, PackedNeighborIter};
pub use writer::{WriteSummary, write_packed};

pub(crate) use dataset::canonical_edges;
pub use dataset::dataset_checksum;
pub use format::StableHasher;
pub(crate) use format::checksum;

use crate::{EdgeKind, NodeId};

/// One adjacent node and the relationship kind connecting it.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct Neighbor {
    pub node: NodeId,
    pub kind: EdgeKind,
}

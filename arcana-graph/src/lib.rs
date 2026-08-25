//! Repository-agnostic graph engine shared by Arcana and future consumers.

pub mod primitives;
pub mod snapshot;
pub mod storage;
pub mod traversal;

pub use primitives::{Edge, EdgeKind, GraphDataset, NodeId};

#[cfg(test)]
mod traversal_tests;

//! Immutable graph snapshots and delta overlays.

mod compaction;
mod error;
mod graph;
mod graph_error;
mod manifest;
mod manifest_io;
mod overlay;
mod overlay_error;
mod overlay_format;
mod overlay_validation;
mod overlay_writer;

#[cfg(test)]
mod compaction_tests;
#[cfg(test)]
mod graph_tests;
#[cfg(test)]
mod manifest_tests;
#[cfg(test)]
mod overlay_tests;

pub use crate::storage::{Direction, Neighbor};
pub use compaction::compact_snapshot;
pub use error::ManifestError;
pub use graph::{GraphSnapshot, VisibleNeighborIter, derive_snapshot_id, publish_snapshot};
pub use graph_error::SnapshotError;
pub use manifest::{MANIFEST_VERSION, Manifest, SnapshotManifest};
pub use manifest_io::{read_manifest, write_manifest};
pub use overlay::{GraphOverlay, OverlayChanges, OverlayWriteSummary};
pub use overlay_error::OverlayError;
pub use overlay_writer::write_overlay;

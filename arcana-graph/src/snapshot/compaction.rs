use std::fs;
use std::io;
use std::path::Path;

use crate::storage::write_packed;
use crate::{Edge, GraphDataset, NodeId};

use super::graph::{manifest_parent, validate_component_path};
use super::{GraphSnapshot, SnapshotError, SnapshotManifest, publish_snapshot};

/// Compacts one snapshot into a new packed base and base-only manifest.
///
/// The source snapshot is never modified. The new packed file is written and
/// validated before the new manifest is published.
pub fn compact_snapshot(
    source_manifest: impl AsRef<Path>,
    output_manifest: impl AsRef<Path>,
    output_base_file: impl AsRef<Path>,
    created_unix_seconds: u64,
) -> Result<SnapshotManifest, SnapshotError> {
    let source_manifest = source_manifest.as_ref();
    let output_manifest = output_manifest.as_ref();
    let output_base_file = validate_component_path("base_file", output_base_file.as_ref())?;
    if output_manifest.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "snapshot manifest already exists: {}",
                output_manifest.display()
            ),
        )
        .into());
    }

    let output_base = manifest_parent(output_manifest).join(&output_base_file);
    let source = GraphSnapshot::open(source_manifest)?;
    let dataset = materialize(&source)?;
    let summary = write_packed(&output_base, &dataset)?;
    if summary.edge_count != source.edge_count() {
        let _ = fs::remove_file(&output_base);
        return Err(SnapshotError::CompactionMismatch {
            field: "edge_count",
            expected: source.edge_count(),
            actual: summary.edge_count,
        });
    }
    if summary.dataset_checksum != source.dataset_checksum() {
        let _ = fs::remove_file(&output_base);
        return Err(SnapshotError::CompactionMismatch {
            field: "dataset_checksum",
            expected: source.dataset_checksum(),
            actual: summary.dataset_checksum,
        });
    }

    match publish_snapshot(
        output_manifest,
        &output_base_file,
        None,
        created_unix_seconds,
    ) {
        Ok(manifest) => Ok(manifest),
        Err(error) => {
            let _ = fs::remove_file(output_base);
            Err(error)
        }
    }
}

fn materialize(snapshot: &GraphSnapshot) -> Result<GraphDataset, SnapshotError> {
    let capacity =
        usize::try_from(snapshot.edge_count()).map_err(|_| SnapshotError::SizeOverflow)?;
    let mut edges = Vec::with_capacity(capacity);
    for source in 0..snapshot.node_count() {
        for neighbor in snapshot.forward_neighbors(NodeId(source))? {
            edges.push(Edge {
                source: NodeId(source),
                target: neighbor.node,
                kind: neighbor.kind,
            });
        }
    }
    Ok(GraphDataset {
        node_count: snapshot.node_count(),
        edges,
    })
}

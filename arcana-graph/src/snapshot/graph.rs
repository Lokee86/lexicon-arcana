use std::path::{Component, Path, PathBuf};

use crate::storage::{
    Direction, Neighbor, PackedGraph, PackedNeighborIter, QueryError, StableHasher,
};
use crate::{Edge, GraphDataset, NodeId};

use super::{GraphOverlay, SnapshotError, SnapshotManifest, read_manifest, write_manifest};

#[derive(Clone, Debug)]
pub struct GraphSnapshot {
    manifest: SnapshotManifest,
    base: PackedGraph,
    overlay: Option<GraphOverlay>,
}

#[derive(Debug)]
pub enum VisibleNeighborIter<'a> {
    Base(PackedNeighborIter<'a>),
    Owned(std::vec::IntoIter<Neighbor>),
}

impl Iterator for VisibleNeighborIter<'_> {
    type Item = Neighbor;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Base(iter) => iter.next(),
            Self::Owned(iter) => iter.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Base(iter) => iter.size_hint(),
            Self::Owned(iter) => iter.size_hint(),
        }
    }
}

impl ExactSizeIterator for VisibleNeighborIter<'_> {}

impl GraphSnapshot {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SnapshotError> {
        let path = path.as_ref();
        let manifest = read_manifest(path)?;
        let parent = manifest_parent(path);
        let base = PackedGraph::open(parent.join(&manifest.base_file))?;
        validate_base_manifest(&manifest, &base)?;
        let overlay = manifest
            .overlay_file
            .as_ref()
            .map(|overlay_file| GraphOverlay::open(parent.join(overlay_file), &base))
            .transpose()?;
        validate_visible_manifest(&manifest, &base, overlay.as_ref())?;
        Ok(Self {
            manifest,
            base,
            overlay,
        })
    }

    pub const fn manifest(&self) -> &SnapshotManifest {
        &self.manifest
    }

    pub const fn snapshot_id(&self) -> u64 {
        self.manifest.snapshot_id
    }

    pub const fn node_count(&self) -> u32 {
        self.manifest.node_count
    }

    pub const fn edge_count(&self) -> u64 {
        self.manifest.visible_edge_count
    }

    pub const fn dataset_checksum(&self) -> u64 {
        self.manifest.visible_dataset_checksum
    }

    pub fn forward_neighbors(&self, node: NodeId) -> Result<Vec<Neighbor>, QueryError> {
        self.neighbors_owned(node, Direction::Forward)
    }

    pub fn reverse_neighbors(&self, node: NodeId) -> Result<Vec<Neighbor>, QueryError> {
        self.neighbors_owned(node, Direction::Reverse)
    }

    pub fn forward_neighbors_iter(
        &self,
        node: NodeId,
    ) -> Result<VisibleNeighborIter<'_>, QueryError> {
        self.neighbors_iter(node, Direction::Forward)
    }

    pub fn reverse_neighbors_iter(
        &self,
        node: NodeId,
    ) -> Result<VisibleNeighborIter<'_>, QueryError> {
        self.neighbors_iter(node, Direction::Reverse)
    }

    pub fn materialize_base_dataset(&self) -> Result<GraphDataset, QueryError> {
        let mut edges = Vec::with_capacity(self.manifest.base_edge_count as usize);
        for source in 0..self.base.node_count() {
            for neighbor in self.base.forward_neighbors_iter(NodeId(source))? {
                edges.push(Edge {
                    source: NodeId(source),
                    target: neighbor.node,
                    kind: neighbor.kind,
                });
            }
        }
        Ok(GraphDataset {
            node_count: self.base.node_count(),
            edges,
        })
    }

    fn neighbors_owned(
        &self,
        node: NodeId,
        direction: Direction,
    ) -> Result<Vec<Neighbor>, QueryError> {
        let base_neighbors = match direction {
            Direction::Forward => self.base.forward_neighbors(node)?,
            Direction::Reverse => self.base.reverse_neighbors(node)?,
        };
        match &self.overlay {
            Some(overlay) => Ok(overlay.merge_owned(direction, node, base_neighbors)),
            None => Ok(base_neighbors),
        }
    }

    fn neighbors_iter(
        &self,
        node: NodeId,
        direction: Direction,
    ) -> Result<VisibleNeighborIter<'_>, QueryError> {
        match &self.overlay {
            Some(overlay) => {
                let base_neighbors = match direction {
                    Direction::Forward => self.base.forward_neighbors(node)?,
                    Direction::Reverse => self.base.reverse_neighbors(node)?,
                };
                Ok(VisibleNeighborIter::Owned(
                    overlay
                        .merge_owned(direction, node, base_neighbors)
                        .into_iter(),
                ))
            }
            None => {
                let base_neighbors = match direction {
                    Direction::Forward => self.base.forward_neighbors_iter(node)?,
                    Direction::Reverse => self.base.reverse_neighbors_iter(node)?,
                };
                Ok(VisibleNeighborIter::Base(base_neighbors))
            }
        }
    }
}

pub fn publish_snapshot(
    manifest_path: impl AsRef<Path>,
    base_file: impl AsRef<Path>,
    overlay_file: Option<&Path>,
    created_unix_seconds: u64,
) -> Result<SnapshotManifest, SnapshotError> {
    let manifest_path = manifest_path.as_ref();
    let base_file = validate_component_path("base_file", base_file.as_ref())?;
    let overlay_file = overlay_file
        .map(|path| validate_component_path("overlay_file", path))
        .transpose()?;
    let parent = manifest_parent(manifest_path);
    let base = PackedGraph::open(parent.join(&base_file))?;
    let overlay = overlay_file
        .as_ref()
        .map(|path| GraphOverlay::open(parent.join(path), &base))
        .transpose()?;
    let visible_edge_count = overlay
        .as_ref()
        .map_or(base.edge_count(), GraphOverlay::visible_edge_count);
    let visible_dataset_checksum = overlay.as_ref().map_or(
        base.dataset_checksum(),
        GraphOverlay::visible_dataset_checksum,
    );
    let overlay_checksum = overlay.as_ref().map_or(0, GraphOverlay::overlay_checksum);
    let manifest = SnapshotManifest {
        snapshot_id: derive_snapshot_id(
            base.node_count(),
            visible_edge_count,
            visible_dataset_checksum,
        ),
        created_unix_seconds,
        node_count: base.node_count(),
        base_edge_count: base.edge_count(),
        visible_edge_count,
        base_dataset_checksum: base.dataset_checksum(),
        overlay_checksum,
        visible_dataset_checksum,
        base_file,
        overlay_file,
    };
    write_manifest(manifest_path, &manifest)?;
    Ok(manifest)
}

pub fn derive_snapshot_id(
    node_count: u32,
    visible_edge_count: u64,
    visible_dataset_checksum: u64,
) -> u64 {
    let mut hasher = StableHasher::new();
    hasher.update(b"arcana-snapshot-v1");
    hasher.update(&node_count.to_le_bytes());
    hasher.update(&visible_edge_count.to_le_bytes());
    hasher.update(&visible_dataset_checksum.to_le_bytes());
    hasher.finish()
}

fn validate_base_manifest(
    manifest: &SnapshotManifest,
    base: &PackedGraph,
) -> Result<(), SnapshotError> {
    compare(
        "node_count",
        u64::from(manifest.node_count),
        u64::from(base.node_count()),
    )?;
    compare(
        "base_edge_count",
        manifest.base_edge_count,
        base.edge_count(),
    )?;
    compare(
        "base_dataset_checksum",
        manifest.base_dataset_checksum,
        base.dataset_checksum(),
    )
}

fn validate_visible_manifest(
    manifest: &SnapshotManifest,
    base: &PackedGraph,
    overlay: Option<&GraphOverlay>,
) -> Result<(), SnapshotError> {
    let actual_overlay_checksum = overlay.map_or(0, GraphOverlay::overlay_checksum);
    let actual_edge_count = overlay.map_or(base.edge_count(), GraphOverlay::visible_edge_count);
    let actual_dataset_checksum = overlay.map_or(
        base.dataset_checksum(),
        GraphOverlay::visible_dataset_checksum,
    );
    compare(
        "overlay_checksum",
        manifest.overlay_checksum,
        actual_overlay_checksum,
    )?;
    compare(
        "visible_edge_count",
        manifest.visible_edge_count,
        actual_edge_count,
    )?;
    compare(
        "visible_dataset_checksum",
        manifest.visible_dataset_checksum,
        actual_dataset_checksum,
    )?;
    compare(
        "snapshot_id",
        manifest.snapshot_id,
        derive_snapshot_id(
            manifest.node_count,
            manifest.visible_edge_count,
            manifest.visible_dataset_checksum,
        ),
    )
}

fn compare(field: &'static str, expected: u64, actual: u64) -> Result<(), SnapshotError> {
    if expected == actual {
        Ok(())
    } else {
        Err(SnapshotError::ManifestMismatch {
            field,
            expected,
            actual,
        })
    }
}

pub(super) fn validate_component_path(
    field: &'static str,
    path: &Path,
) -> Result<PathBuf, SnapshotError> {
    let text = path.to_string_lossy();
    if text.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(SnapshotError::InvalidRelativePath {
            field,
            path: text.into_owned(),
        });
    }
    Ok(path.to_path_buf())
}

pub(super) fn manifest_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

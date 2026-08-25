use std::borrow::Cow;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Edge, GraphDataset};

use super::dataset::{canonical_edges, dataset_checksum};
use super::format::{HEADER_LEN, Header, Layout, StableHasher};
use super::{DatasetError, Direction, PackedError};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Metadata produced after a packed file is committed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WriteSummary {
    pub node_count: u32,
    pub edge_count: u64,
    pub dataset_checksum: u64,
    pub file_len: u64,
}

/// Writes a new immutable packed graph and refuses to replace an existing path.
pub fn write_packed(
    path: impl AsRef<Path>,
    dataset: &GraphDataset,
) -> Result<WriteSummary, PackedError> {
    let path = path.as_ref();
    if path.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("packed graph already exists: {}", path.display()),
        )
        .into());
    }

    let temp_path = temporary_path(path);
    let result = write_then_commit(path, &temp_path, dataset);
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn write_then_commit(
    path: &Path,
    temp_path: &Path,
    dataset: &GraphDataset,
) -> Result<WriteSummary, PackedError> {
    let edges = edges_for_write(dataset)?;
    let edge_count = u64::try_from(edges.len()).map_err(|_| PackedError::SizeOverflow)?;
    let layout = Layout::for_counts(dataset.node_count, edge_count)?;
    let dataset_checksum = dataset_checksum(dataset.node_count, &edges);

    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(&[0_u8; HEADER_LEN as usize])?;

    let mut payload = PayloadWriter::new(&mut writer, u64::from(HEADER_LEN));
    write_direction(
        &mut payload,
        &edges,
        dataset.node_count,
        Direction::Forward,
        &layout,
    )?;

    let mut reverse_edges = edges.into_owned();
    reverse_edges.sort_unstable_by_key(|edge| (edge.target, edge.source, edge.kind));
    write_direction(
        &mut payload,
        &reverse_edges,
        dataset.node_count,
        Direction::Reverse,
        &layout,
    )?;
    payload.pad_to(layout.file_len)?;
    let payload_checksum = payload.finish();

    let header = Header {
        node_count: dataset.node_count,
        edge_count,
        dataset_checksum,
        payload_checksum,
        layout,
    };
    writer.seek(SeekFrom::Start(0))?;
    writer.write_all(&header.encode())?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    drop(writer);

    fs::rename(temp_path, path)?;
    Ok(WriteSummary {
        node_count: dataset.node_count,
        edge_count,
        dataset_checksum,
        file_len: layout.file_len,
    })
}

fn edges_for_write(dataset: &GraphDataset) -> Result<Cow<'_, [Edge]>, DatasetError> {
    if is_canonical(dataset)? {
        Ok(Cow::Borrowed(&dataset.edges))
    } else {
        canonical_edges(dataset).map(Cow::Owned)
    }
}

fn is_canonical(dataset: &GraphDataset) -> Result<bool, DatasetError> {
    let mut previous = None;
    for &edge in &dataset.edges {
        if edge.source.0 >= dataset.node_count || edge.target.0 >= dataset.node_count {
            return Err(DatasetError::EndpointOutOfRange {
                edge,
                node_count: dataset.node_count,
            });
        }
        if let Some(previous) = previous {
            if previous == edge {
                return Err(DatasetError::DuplicateEdge { edge });
            }
            if previous > edge {
                return Ok(false);
            }
        }
        previous = Some(edge);
    }
    Ok(true)
}

fn write_direction(
    writer: &mut PayloadWriter<'_>,
    edges: &[Edge],
    node_count: u32,
    direction: Direction,
    layout: &Layout,
) -> Result<(), PackedError> {
    let (offset_at, node_at, kind_at) = match direction {
        Direction::Forward => (
            layout.forward_offsets,
            layout.forward_targets,
            layout.forward_kinds,
        ),
        Direction::Reverse => (
            layout.reverse_offsets,
            layout.reverse_sources,
            layout.reverse_kinds,
        ),
    };

    writer.pad_to(offset_at)?;
    for offset in offsets(edges, node_count, direction) {
        writer.write(&offset.to_le_bytes())?;
    }

    writer.pad_to(node_at)?;
    for edge in edges {
        let node = match direction {
            Direction::Forward => edge.target,
            Direction::Reverse => edge.source,
        };
        writer.write(&node.0.to_le_bytes())?;
    }

    writer.pad_to(kind_at)?;
    for edge in edges {
        writer.write(&edge.kind.0.to_le_bytes())?;
    }
    Ok(())
}

fn offsets(edges: &[Edge], node_count: u32, direction: Direction) -> Vec<u64> {
    let mut offsets = vec![0_u64; node_count as usize + 1];
    for edge in edges {
        let node = match direction {
            Direction::Forward => edge.source,
            Direction::Reverse => edge.target,
        };
        offsets[node.0 as usize + 1] += 1;
    }
    for index in 1..offsets.len() {
        offsets[index] += offsets[index - 1];
    }
    offsets
}

struct PayloadWriter<'a> {
    writer: &'a mut BufWriter<File>,
    hasher: StableHasher,
    position: u64,
}

impl<'a> PayloadWriter<'a> {
    fn new(writer: &'a mut BufWriter<File>, position: u64) -> Self {
        Self {
            writer,
            hasher: StableHasher::new(),
            position,
        }
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), PackedError> {
        self.writer.write_all(bytes)?;
        self.hasher.update(bytes);
        self.position = self
            .position
            .checked_add(bytes.len() as u64)
            .ok_or(PackedError::SizeOverflow)?;
        Ok(())
    }

    fn pad_to(&mut self, target: u64) -> Result<(), PackedError> {
        if self.position > target {
            return Err(PackedError::SizeOverflow);
        }
        let zeros = [0_u8; 8];
        while self.position < target {
            let count = usize::try_from((target - self.position).min(8))
                .map_err(|_| PackedError::SizeOverflow)?;
            self.write(&zeros[..count])?;
        }
        Ok(())
    }

    fn finish(self) -> u64 {
        self.hasher.finish()
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut name = path
        .file_name()
        .map_or_else(|| OsString::from("arcana"), OsString::from);
    name.push(format!(".tmp.{}.{}", std::process::id(), sequence));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EdgeKind, NodeId};

    #[test]
    fn canonical_compiler_output_uses_borrowed_forward_edges() {
        let dataset = GraphDataset {
            node_count: 3,
            edges: vec![
                Edge {
                    source: NodeId(0),
                    target: NodeId(1),
                    kind: EdgeKind(1),
                },
                Edge {
                    source: NodeId(1),
                    target: NodeId(2),
                    kind: EdgeKind(2),
                },
            ],
        };

        assert!(matches!(
            edges_for_write(&dataset).unwrap(),
            Cow::Borrowed(_)
        ));

        let mut reversed = dataset.clone();
        reversed.edges.reverse();
        assert!(matches!(edges_for_write(&reversed).unwrap(), Cow::Owned(_)));
    }
}

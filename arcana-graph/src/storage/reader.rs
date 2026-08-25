use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::{EdgeKind, NodeId};

use super::format::{
    HEADER_LEN, Header, Layout, StableHasher, checksum, get_u16, get_u32, get_u64,
};
use super::{Direction, Neighbor, PackedError, QueryError};

/// Validated packed graph backed by one immutable byte buffer.
#[derive(Clone, Debug)]
pub struct PackedGraph {
    // A read-only mmap would require unsafe code (directly or through an mmap
    // implementation), which is incompatible with Arcana's unsafe-code lint.
    bytes: Arc<[u8]>,
    header: Header,
}

impl PackedGraph {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PackedError> {
        let bytes: Arc<[u8]> = fs::read(path)?.into();
        let header = Header::decode(&bytes)?;
        validate_file(&bytes, header)?;
        Ok(Self { bytes, header })
    }

    pub const fn node_count(&self) -> u32 {
        self.header.node_count
    }

    pub const fn edge_count(&self) -> u64 {
        self.header.edge_count
    }

    pub const fn dataset_checksum(&self) -> u64 {
        self.header.dataset_checksum
    }

    pub fn forward_neighbors(&self, node: NodeId) -> Result<Vec<Neighbor>, QueryError> {
        self.neighbors(node, Direction::Forward)
    }

    pub fn reverse_neighbors(&self, node: NodeId) -> Result<Vec<Neighbor>, QueryError> {
        self.neighbors(node, Direction::Reverse)
    }

    pub fn forward_neighbors_iter(
        &self,
        node: NodeId,
    ) -> Result<PackedNeighborIter<'_>, QueryError> {
        self.neighbors_iter(node, Direction::Forward)
    }

    pub fn reverse_neighbors_iter(
        &self,
        node: NodeId,
    ) -> Result<PackedNeighborIter<'_>, QueryError> {
        self.neighbors_iter(node, Direction::Reverse)
    }

    pub fn neighbors_iter(
        &self,
        node: NodeId,
        direction: Direction,
    ) -> Result<PackedNeighborIter<'_>, QueryError> {
        if node.0 >= self.header.node_count {
            return Err(QueryError::InvalidNode {
                node,
                node_count: self.header.node_count,
            });
        }

        let (offset_base, node_base, kind_base) = section_bases(self.header.layout, direction);
        let start = read_offset(&self.bytes, offset_base, node.0);
        let end = read_offset(&self.bytes, offset_base, node.0 + 1);
        Ok(PackedNeighborIter {
            bytes: &self.bytes,
            node_base,
            kind_base,
            next: start,
            end,
        })
    }

    fn neighbors(&self, node: NodeId, direction: Direction) -> Result<Vec<Neighbor>, QueryError> {
        Ok(self.neighbors_iter(node, direction)?.collect())
    }
}

/// Borrowed iterator over one node's packed adjacency section.
#[derive(Debug)]
pub struct PackedNeighborIter<'a> {
    bytes: &'a [u8],
    node_base: u64,
    kind_base: u64,
    next: u64,
    end: u64,
}

impl Iterator for PackedNeighborIter<'_> {
    type Item = Neighbor;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.end {
            return None;
        }
        let index = self.next;
        self.next += 1;
        Some(Neighbor {
            node: NodeId(read_node(self.bytes, self.node_base, index)),
            kind: EdgeKind(read_kind(self.bytes, self.kind_base, index)),
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.end - self.next) as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for PackedNeighborIter<'_> {}

fn validate_file(bytes: &[u8], header: Header) -> Result<(), PackedError> {
    let actual = bytes.len() as u64;
    if header.layout.file_len != actual {
        return Err(PackedError::FileLengthMismatch {
            declared: header.layout.file_len,
            actual,
        });
    }

    let expected = Layout::for_counts(header.node_count, header.edge_count)?;
    compare_layout(header.layout, expected)?;

    let payload = &bytes[usize::from(HEADER_LEN)..];
    let payload_checksum = checksum(payload);
    if payload_checksum != header.payload_checksum {
        return Err(PackedError::PayloadChecksumMismatch {
            expected: header.payload_checksum,
            actual: payload_checksum,
        });
    }

    let dataset_checksum = validate_direction(bytes, header, Direction::Forward)?
        .expect("forward validation returns a checksum");
    if dataset_checksum != header.dataset_checksum {
        return Err(PackedError::DatasetChecksumMismatch {
            expected: header.dataset_checksum,
            actual: dataset_checksum,
        });
    }
    validate_direction(bytes, header, Direction::Reverse)?;
    Ok(())
}

fn compare_layout(actual: Layout, expected: Layout) -> Result<(), PackedError> {
    let sections = [
        (
            "forward offsets",
            actual.forward_offsets,
            expected.forward_offsets,
        ),
        (
            "forward targets",
            actual.forward_targets,
            expected.forward_targets,
        ),
        (
            "forward kinds",
            actual.forward_kinds,
            expected.forward_kinds,
        ),
        (
            "reverse offsets",
            actual.reverse_offsets,
            expected.reverse_offsets,
        ),
        (
            "reverse sources",
            actual.reverse_sources,
            expected.reverse_sources,
        ),
        (
            "reverse kinds",
            actual.reverse_kinds,
            expected.reverse_kinds,
        ),
    ];
    for (section, actual, expected) in sections {
        if actual != expected {
            return Err(PackedError::LayoutMismatch { section });
        }
    }
    if actual.file_len != expected.file_len {
        return Err(PackedError::LayoutMismatch {
            section: "file length",
        });
    }
    Ok(())
}

fn validate_direction(
    bytes: &[u8],
    header: Header,
    direction: Direction,
) -> Result<Option<u64>, PackedError> {
    let (offset_base, node_base, kind_base) = section_bases(header.layout, direction);
    if read_offset(bytes, offset_base, 0) != 0
        || read_offset(bytes, offset_base, header.node_count) != header.edge_count
    {
        return Err(PackedError::InvalidOffsetTable { direction });
    }

    let mut checksum = (direction == Direction::Forward).then(StableHasher::new);
    if let Some(hasher) = &mut checksum {
        hasher.update(&header.node_count.to_le_bytes());
        hasher.update(&header.edge_count.to_le_bytes());
    }

    let mut previous_end = 0;
    for node in 0..header.node_count {
        let start = read_offset(bytes, offset_base, node);
        let end = read_offset(bytes, offset_base, node + 1);
        if start != previous_end || end < start || end > header.edge_count {
            return Err(PackedError::InvalidOffsetTable { direction });
        }
        previous_end = end;

        let mut previous = None;
        for index in start..end {
            let adjacent = read_node(bytes, node_base, index);
            let kind = read_kind(bytes, kind_base, index);
            if adjacent >= header.node_count {
                return Err(PackedError::InvalidNeighbor {
                    direction,
                    node: NodeId(node),
                    neighbor: NodeId(adjacent),
                    node_count: header.node_count,
                });
            }
            if previous.is_some_and(|value| value >= (adjacent, kind)) {
                return Err(PackedError::UnsortedAdjacency {
                    direction,
                    node: NodeId(node),
                });
            }
            previous = Some((adjacent, kind));

            if let Some(hasher) = &mut checksum {
                hasher.update(&node.to_le_bytes());
                hasher.update(&adjacent.to_le_bytes());
                hasher.update(&kind.to_le_bytes());
            }
        }
    }
    Ok(checksum.map(|hasher| hasher.finish()))
}

fn section_bases(layout: Layout, direction: Direction) -> (u64, u64, u64) {
    match direction {
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
    }
}

fn read_offset(bytes: &[u8], base: u64, node: u32) -> u64 {
    get_u64(bytes, (base + u64::from(node) * 8) as usize)
}

fn read_node(bytes: &[u8], base: u64, index: u64) -> u32 {
    get_u32(bytes, (base + index * 4) as usize)
}

fn read_kind(bytes: &[u8], base: u64, index: u64) -> u16 {
    get_u16(bytes, (base + index * 2) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloned_packed_graphs_share_immutable_backing_bytes() {
        let layout = Layout::for_counts(0, 0).unwrap();
        let bytes: Arc<[u8]> = vec![0; layout.file_len as usize].into();
        let graph = PackedGraph {
            bytes,
            header: Header {
                node_count: 0,
                edge_count: 0,
                dataset_checksum: 0,
                payload_checksum: 0,
                layout,
            },
        };
        let clone = graph.clone();

        assert!(Arc::ptr_eq(&graph.bytes, &clone.bytes));
    }
}

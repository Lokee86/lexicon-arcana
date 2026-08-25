use crate::Edge;
use crate::storage::StableHasher;

use super::OverlayError;

pub(super) const MAGIC: [u8; 8] = *b"ARCOVL01";
pub(super) const FORMAT_VERSION: u16 = 1;
pub(super) const HEADER_LEN: u16 = 112;
const FLAGS: u32 = 0;
const EDGE_LEN: u64 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OverlayLayout {
    pub added_edges: u64,
    pub removed_edges: u64,
    pub file_len: u64,
}

impl OverlayLayout {
    pub fn for_counts(added_count: u64, removed_count: u64) -> Result<Self, OverlayError> {
        let added_edges = u64::from(HEADER_LEN);
        let removed_edges = section_end(added_edges, edge_bytes(added_count)?)?;
        let file_len = section_end(removed_edges, edge_bytes(removed_count)?)?;
        Ok(Self {
            added_edges,
            removed_edges,
            file_len,
        })
    }
}

fn edge_bytes(count: u64) -> Result<u64, OverlayError> {
    count
        .checked_mul(EDGE_LEN)
        .ok_or(OverlayError::SizeOverflow)
}

fn section_end(start: u64, length: u64) -> Result<u64, OverlayError> {
    start
        .checked_add(length)
        .and_then(|end| end.checked_add(7))
        .map(|end| end & !7)
        .ok_or(OverlayError::SizeOverflow)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct OverlayHeader {
    pub node_count: u32,
    pub base_edge_count: u64,
    pub added_count: u64,
    pub removed_count: u64,
    pub base_dataset_checksum: u64,
    pub visible_edge_count: u64,
    pub visible_dataset_checksum: u64,
    pub overlay_checksum: u64,
    pub payload_checksum: u64,
    pub layout: OverlayLayout,
}

impl OverlayHeader {
    pub fn encode(self) -> [u8; HEADER_LEN as usize] {
        let mut bytes = [0_u8; HEADER_LEN as usize];
        bytes[0..8].copy_from_slice(&MAGIC);
        put_u16(&mut bytes, 8, FORMAT_VERSION);
        put_u16(&mut bytes, 10, HEADER_LEN);
        put_u32(&mut bytes, 12, FLAGS);
        put_u32(&mut bytes, 16, self.node_count);
        put_u32(&mut bytes, 20, 0);
        put_u64(&mut bytes, 24, self.base_edge_count);
        put_u64(&mut bytes, 32, self.added_count);
        put_u64(&mut bytes, 40, self.removed_count);
        put_u64(&mut bytes, 48, self.base_dataset_checksum);
        put_u64(&mut bytes, 56, self.visible_edge_count);
        put_u64(&mut bytes, 64, self.visible_dataset_checksum);
        put_u64(&mut bytes, 72, self.overlay_checksum);
        put_u64(&mut bytes, 80, self.payload_checksum);
        put_u64(&mut bytes, 88, self.layout.added_edges);
        put_u64(&mut bytes, 96, self.layout.removed_edges);
        put_u64(&mut bytes, 104, self.layout.file_len);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, OverlayError> {
        if bytes.len() < usize::from(HEADER_LEN) {
            return Err(OverlayError::FileTooShort {
                actual: bytes.len() as u64,
                minimum: u64::from(HEADER_LEN),
            });
        }
        if bytes[0..8] != MAGIC {
            return Err(OverlayError::InvalidMagic);
        }
        let version = get_u16(bytes, 8);
        if version != FORMAT_VERSION {
            return Err(OverlayError::UnsupportedVersion { found: version });
        }
        let header_len = get_u16(bytes, 10);
        if header_len != HEADER_LEN {
            return Err(OverlayError::InvalidHeaderLength { found: header_len });
        }
        let flags = get_u32(bytes, 12);
        if flags != FLAGS {
            return Err(OverlayError::UnsupportedFlags { found: flags });
        }
        let reserved = get_u32(bytes, 20);
        if reserved != 0 {
            return Err(OverlayError::ReservedFieldSet { found: reserved });
        }
        Ok(Self {
            node_count: get_u32(bytes, 16),
            base_edge_count: get_u64(bytes, 24),
            added_count: get_u64(bytes, 32),
            removed_count: get_u64(bytes, 40),
            base_dataset_checksum: get_u64(bytes, 48),
            visible_edge_count: get_u64(bytes, 56),
            visible_dataset_checksum: get_u64(bytes, 64),
            overlay_checksum: get_u64(bytes, 72),
            payload_checksum: get_u64(bytes, 80),
            layout: OverlayLayout {
                added_edges: get_u64(bytes, 88),
                removed_edges: get_u64(bytes, 96),
                file_len: get_u64(bytes, 104),
            },
        })
    }
}

pub(super) fn operation_checksum(
    node_count: u32,
    base_edge_count: u64,
    base_checksum: u64,
    added: &[Edge],
    removed: &[Edge],
) -> u64 {
    let mut hasher = StableHasher::new();
    hasher.update(b"arcana-overlay-v1");
    hasher.update(&node_count.to_le_bytes());
    hasher.update(&base_edge_count.to_le_bytes());
    hasher.update(&base_checksum.to_le_bytes());
    hasher.update(&(added.len() as u64).to_le_bytes());
    hasher.update(&(removed.len() as u64).to_le_bytes());
    hash_edges(&mut hasher, added);
    hash_edges(&mut hasher, removed);
    hasher.finish()
}

fn hash_edges(hasher: &mut StableHasher, edges: &[Edge]) {
    for edge in edges {
        hasher.update(&edge.source.0.to_le_bytes());
        hasher.update(&edge.target.0.to_le_bytes());
        hasher.update(&edge.kind.0.to_le_bytes());
    }
}

pub(super) fn get_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated range"),
    )
}

pub(super) fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated range"),
    )
}

pub(super) fn get_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("validated range"),
    )
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

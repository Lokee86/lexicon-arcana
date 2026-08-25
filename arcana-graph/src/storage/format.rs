use super::PackedError;

pub(super) const MAGIC: [u8; 8] = *b"ARCGPH01";
pub(super) const FORMAT_VERSION: u16 = 1;
pub(super) const HEADER_LEN: u16 = 128;
pub(super) const FLAGS: u32 = 0;
pub(super) const ENDIAN_MARKER: u64 = 0x0102_0304_0506_0708;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Layout {
    pub forward_offsets: u64,
    pub forward_targets: u64,
    pub forward_kinds: u64,
    pub reverse_offsets: u64,
    pub reverse_sources: u64,
    pub reverse_kinds: u64,
    pub file_len: u64,
}

impl Layout {
    pub fn for_counts(node_count: u32, edge_count: u64) -> Result<Self, PackedError> {
        let offset_bytes = u64::from(node_count)
            .checked_add(1)
            .and_then(|count| count.checked_mul(8))
            .ok_or(PackedError::SizeOverflow)?;
        let node_bytes = edge_count.checked_mul(4).ok_or(PackedError::SizeOverflow)?;
        let kind_bytes = edge_count.checked_mul(2).ok_or(PackedError::SizeOverflow)?;

        let forward_offsets = u64::from(HEADER_LEN);
        let forward_targets = section_end(forward_offsets, offset_bytes)?;
        let forward_kinds = section_end(forward_targets, node_bytes)?;
        let reverse_offsets = section_end(forward_kinds, kind_bytes)?;
        let reverse_sources = section_end(reverse_offsets, offset_bytes)?;
        let reverse_kinds = section_end(reverse_sources, node_bytes)?;
        let file_len = section_end(reverse_kinds, kind_bytes)?;

        Ok(Self {
            forward_offsets,
            forward_targets,
            forward_kinds,
            reverse_offsets,
            reverse_sources,
            reverse_kinds,
            file_len,
        })
    }
}

fn section_end(start: u64, length: u64) -> Result<u64, PackedError> {
    start
        .checked_add(length)
        .and_then(|end| end.checked_add(7))
        .map(|end| end & !7)
        .ok_or(PackedError::SizeOverflow)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Header {
    pub node_count: u32,
    pub edge_count: u64,
    pub dataset_checksum: u64,
    pub payload_checksum: u64,
    pub layout: Layout,
}

impl Header {
    pub fn encode(self) -> [u8; HEADER_LEN as usize] {
        let mut bytes = [0_u8; HEADER_LEN as usize];
        bytes[0..8].copy_from_slice(&MAGIC);
        put_u16(&mut bytes, 8, FORMAT_VERSION);
        put_u16(&mut bytes, 10, HEADER_LEN);
        put_u32(&mut bytes, 12, FLAGS);
        put_u32(&mut bytes, 16, self.node_count);
        put_u64(&mut bytes, 24, self.edge_count);
        put_u64(&mut bytes, 32, self.dataset_checksum);
        put_u64(&mut bytes, 40, self.payload_checksum);
        put_u64(&mut bytes, 48, self.layout.forward_offsets);
        put_u64(&mut bytes, 56, self.layout.forward_targets);
        put_u64(&mut bytes, 64, self.layout.forward_kinds);
        put_u64(&mut bytes, 72, self.layout.reverse_offsets);
        put_u64(&mut bytes, 80, self.layout.reverse_sources);
        put_u64(&mut bytes, 88, self.layout.reverse_kinds);
        put_u64(&mut bytes, 96, self.layout.file_len);
        put_u64(&mut bytes, 104, ENDIAN_MARKER);
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PackedError> {
        if bytes.len() < usize::from(HEADER_LEN) {
            return Err(PackedError::FileTooShort {
                actual: bytes.len() as u64,
                minimum: u64::from(HEADER_LEN),
            });
        }
        if bytes[0..8] != MAGIC {
            return Err(PackedError::InvalidMagic);
        }
        let version = get_u16(bytes, 8);
        if version != FORMAT_VERSION {
            return Err(PackedError::UnsupportedVersion { found: version });
        }
        let header_len = get_u16(bytes, 10);
        if header_len != HEADER_LEN {
            return Err(PackedError::InvalidHeaderLength { found: header_len });
        }
        let flags = get_u32(bytes, 12);
        if flags != FLAGS {
            return Err(PackedError::UnsupportedFlags { found: flags });
        }
        let marker = get_u64(bytes, 104);
        if marker != ENDIAN_MARKER {
            return Err(PackedError::InvalidEndianMarker { found: marker });
        }

        Ok(Self {
            node_count: get_u32(bytes, 16),
            edge_count: get_u64(bytes, 24),
            dataset_checksum: get_u64(bytes, 32),
            payload_checksum: get_u64(bytes, 40),
            layout: Layout {
                forward_offsets: get_u64(bytes, 48),
                forward_targets: get_u64(bytes, 56),
                forward_kinds: get_u64(bytes, 64),
                reverse_offsets: get_u64(bytes, 72),
                reverse_sources: get_u64(bytes, 80),
                reverse_kinds: get_u64(bytes, 88),
                file_len: get_u64(bytes, 96),
            },
        })
    }
}

pub struct StableHasher {
    state: u64,
}

impl StableHasher {
    pub fn new() -> Self {
        Self {
            state: 0xcbf2_9ce4_8422_2325,
        }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= u64::from(*byte);
            self.state = self.state.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    pub fn finish(&self) -> u64 {
        self.state
    }
}

pub(crate) fn checksum(bytes: &[u8]) -> u64 {
    let mut hasher = StableHasher::new();
    hasher.update(bytes);
    hasher.finish()
}

pub(super) fn get_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated byte range"),
    )
}

pub(super) fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated byte range"),
    )
}

pub(super) fn get_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("validated byte range"),
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

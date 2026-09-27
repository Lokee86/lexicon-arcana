use super::validation::align_up;
use super::*;

impl RepositoryHeader {
    pub fn layout(
        specs: [SectionSpec; SECTION_COUNT as usize],
        payload_checksum: [u8; SHA256_LEN],
    ) -> Result<Self, FormatError> {
        let mut cursor = u64::from(HEADER_LEN);
        let mut sections = [SectionDescriptor::default(); SECTION_COUNT as usize];
        for kind in SectionKind::ALL {
            cursor = align_up(cursor)?;
            let spec = specs[kind.index()];
            sections[kind.index()] = SectionDescriptor {
                offset: cursor,
                byte_len: spec.byte_len,
                record_count: spec.record_count,
                checksum: spec.checksum,
            };
            cursor = cursor
                .checked_add(spec.byte_len)
                .ok_or(FormatError::SizeOverflow)?;
        }
        let header = Self {
            file_len: align_up(cursor)?,
            payload_checksum,
            sections,
        };
        header.validate()?;
        Ok(header)
    }

    pub fn encode(self) -> [u8; HEADER_LEN as usize] {
        let mut bytes = [0_u8; HEADER_LEN as usize];
        bytes[0..8].copy_from_slice(&MAGIC);
        put_u16(&mut bytes, 8, FORMAT_VERSION);
        put_u16(&mut bytes, 10, HEADER_LEN);
        put_u16(&mut bytes, 12, SECTION_COUNT);
        put_u16(&mut bytes, 14, FLAGS);
        put_u64(&mut bytes, 16, ENDIAN_MARKER);
        put_u64(&mut bytes, 24, self.file_len);
        bytes[32..64].copy_from_slice(&self.payload_checksum);
        for kind in SectionKind::ALL {
            let base = 64 + kind.index() * 56;
            let section = self.sections[kind.index()];
            put_u64(&mut bytes, base, section.offset);
            put_u64(&mut bytes, base + 8, section.byte_len);
            put_u64(&mut bytes, base + 16, section.record_count);
            bytes[base + 24..base + 56].copy_from_slice(&section.checksum);
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FormatError> {
        if bytes.len() < usize::from(HEADER_LEN) {
            return Err(FormatError::FileTooShort);
        }
        if bytes[0..8] != MAGIC {
            return Err(FormatError::InvalidMagic);
        }
        check_eq(
            get_u16(bytes, 8),
            FORMAT_VERSION,
            FormatError::UnsupportedVersion,
        )?;
        check_eq(
            get_u16(bytes, 10),
            HEADER_LEN,
            FormatError::InvalidHeaderLength,
        )?;
        check_eq(
            get_u16(bytes, 12),
            SECTION_COUNT,
            FormatError::InvalidSectionCount,
        )?;
        check_eq(get_u16(bytes, 14), FLAGS, FormatError::UnsupportedFlags)?;
        check_eq(
            get_u64(bytes, 16),
            ENDIAN_MARKER,
            FormatError::InvalidEndianMarker,
        )?;

        let mut sections = [SectionDescriptor::default(); SECTION_COUNT as usize];
        for kind in SectionKind::ALL {
            let base = 64 + kind.index() * 56;
            let mut checksum = [0_u8; SHA256_LEN];
            checksum.copy_from_slice(&bytes[base + 24..base + 56]);
            sections[kind.index()] = SectionDescriptor {
                offset: get_u64(bytes, base),
                byte_len: get_u64(bytes, base + 8),
                record_count: get_u64(bytes, base + 16),
                checksum,
            };
        }
        let mut payload_checksum = [0_u8; SHA256_LEN];
        payload_checksum.copy_from_slice(&bytes[32..64]);
        let header = Self {
            file_len: get_u64(bytes, 24),
            payload_checksum,
            sections,
        };
        header.validate()?;
        Ok(header)
    }

    pub fn section(&self, kind: SectionKind) -> SectionDescriptor {
        self.sections[kind.index()]
    }
}

fn check_eq<T: Copy + Eq>(
    found: T,
    expected: T,
    error: fn(T) -> FormatError,
) -> Result<(), FormatError> {
    (found == expected)
        .then_some(())
        .ok_or_else(|| error(found))
}

fn get_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("header range"))
}

fn get_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("header range"))
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

use sha2::{Digest, Sha256};

use super::RepositoryStoreReadError;
use super::format::{HEADER_LEN, RepositoryHeader, SectionKind};

pub fn section<'a>(bytes: &'a [u8], header: &RepositoryHeader, kind: SectionKind) -> &'a [u8] {
    let descriptor = header.section(kind);
    &bytes[descriptor.offset as usize..(descriptor.offset + descriptor.byte_len) as usize]
}

pub fn verify_checksums(
    bytes: &[u8],
    header: &RepositoryHeader,
) -> Result<(), RepositoryStoreReadError> {
    let payload: [u8; 32] = Sha256::digest(&bytes[usize::from(HEADER_LEN)..]).into();
    if payload != header.payload_checksum {
        return Err(RepositoryStoreReadError::PayloadChecksum);
    }
    for kind in SectionKind::ALL {
        let digest: [u8; 32] = Sha256::digest(section(bytes, header, kind)).into();
        if digest != header.section(kind).checksum {
            return Err(RepositoryStoreReadError::SectionChecksum(kind));
        }
    }
    Ok(())
}

pub fn verify_padding(
    bytes: &[u8],
    header: &RepositoryHeader,
) -> Result<(), RepositoryStoreReadError> {
    let mut cursor = usize::from(HEADER_LEN);
    for kind in SectionKind::ALL {
        let descriptor = header.section(kind);
        if bytes[cursor..descriptor.offset as usize]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(RepositoryStoreReadError::NonZeroPadding);
        }
        cursor = (descriptor.offset + descriptor.byte_len) as usize;
    }
    if bytes[cursor..header.file_len as usize]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(RepositoryStoreReadError::NonZeroPadding);
    }
    Ok(())
}

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::storage::StableHasher;

use super::RepositoryStoreReadError;
use super::format::{HEADER_LEN, RepositoryHeader, SectionKind};

pub(super) fn open_validated(
    path: &Path,
) -> Result<(File, RepositoryHeader, u64), RepositoryStoreReadError> {
    let mut file = File::open(path)?;
    let actual = file.metadata()?.len();
    let mut header_bytes = [0_u8; HEADER_LEN as usize];
    file.read_exact(&mut header_bytes)?;
    let header = RepositoryHeader::decode(&header_bytes)?;
    if actual != header.file_len {
        return Err(RepositoryStoreReadError::FileLength {
            expected: header.file_len,
            actual,
        });
    }

    let mut artifact = StableHasher::new();
    artifact.update(&header_bytes);
    let mut payload = Sha256::new();
    let mut cursor = u64::from(HEADER_LEN);
    let mut buffer = [0_u8; 64 * 1024];

    for kind in SectionKind::ALL {
        let descriptor = header.section(kind);
        let padding = descriptor
            .offset
            .checked_sub(cursor)
            .ok_or(RepositoryStoreReadError::NonZeroPadding)?;
        read_padding(&mut file, padding, &mut buffer, &mut artifact, &mut payload)?;

        let mut section = Sha256::new();
        read_region(&mut file, descriptor.byte_len, &mut buffer, |bytes| {
            artifact.update(bytes);
            payload.update(bytes);
            section.update(bytes);
        })?;
        let digest: [u8; 32] = section.finalize().into();
        if digest != descriptor.checksum {
            return Err(RepositoryStoreReadError::SectionChecksum(kind));
        }
        cursor = descriptor
            .offset
            .checked_add(descriptor.byte_len)
            .ok_or(RepositoryStoreReadError::NonZeroPadding)?;
    }

    let trailing = header
        .file_len
        .checked_sub(cursor)
        .ok_or(RepositoryStoreReadError::NonZeroPadding)?;
    read_padding(
        &mut file,
        trailing,
        &mut buffer,
        &mut artifact,
        &mut payload,
    )?;
    let digest: [u8; 32] = payload.finalize().into();
    if digest != header.payload_checksum {
        return Err(RepositoryStoreReadError::PayloadChecksum);
    }

    file.seek(SeekFrom::Start(0))?;
    Ok((file, header, artifact.finish()))
}

fn read_padding(
    file: &mut File,
    mut len: u64,
    buffer: &mut [u8],
    artifact: &mut StableHasher,
    payload: &mut Sha256,
) -> Result<(), RepositoryStoreReadError> {
    while len > 0 {
        let count = usize::try_from(len.min(buffer.len() as u64))
            .map_err(|_| RepositoryStoreReadError::NonZeroPadding)?;
        file.read_exact(&mut buffer[..count])?;
        let bytes = &buffer[..count];
        if bytes.iter().any(|byte| *byte != 0) {
            return Err(RepositoryStoreReadError::NonZeroPadding);
        }
        artifact.update(bytes);
        payload.update(bytes);
        len -= count as u64;
    }
    Ok(())
}

fn read_region(
    file: &mut File,
    mut len: u64,
    buffer: &mut [u8],
    mut consume: impl FnMut(&[u8]),
) -> Result<(), RepositoryStoreReadError> {
    while len > 0 {
        let count = usize::try_from(len.min(buffer.len() as u64))
            .map_err(|_| RepositoryStoreReadError::InvalidOwnership)?;
        file.read_exact(&mut buffer[..count])?;
        consume(&buffer[..count]);
        len -= count as u64;
    }
    Ok(())
}

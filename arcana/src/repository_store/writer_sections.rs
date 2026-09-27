use std::io::Write;

use super::build::CompactRepositoryBuild;
use super::format::{
    CONTRIBUTION_KIND_OFFSET, CONTRIBUTION_RECORD_INDEX_OFFSET, KIND_INDEX_KIND_OFFSET,
    KIND_INDEX_NODE_ID_OFFSET, OWNERSHIP_CONTRIBUTION_COUNT_OFFSET,
    OWNERSHIP_CONTRIBUTION_START_OFFSET, OWNERSHIP_PATH_ID_OFFSET,
};
use super::record_io::{put_u16, put_u32, put_u64};
use super::writer_sink::SectionSink;
use super::{CompactStringTable, RepositoryStoreWriteError};

pub fn write_strings(
    sink: &mut SectionSink<'_>,
    strings: &CompactStringTable,
) -> Result<(), RepositoryStoreWriteError> {
    let mut offset = 0_u64;
    for value in strings.values() {
        let len = u32::try_from(value.len()).map_err(|_| super::StoreFormatError::StringTooLong)?;
        let mut record = [0_u8; 16];
        put_u64(&mut record, 0, offset);
        put_u32(&mut record, 8, len);
        sink.write_all(&record)?;
        offset = offset
            .checked_add(u64::from(len))
            .ok_or(RepositoryStoreWriteError::SizeOverflow)?;
    }
    for value in strings.values() {
        sink.write_all(value.as_bytes())?;
    }
    Ok(())
}

pub fn write_nodes(
    sink: &mut SectionSink<'_>,
    build: &CompactRepositoryBuild,
) -> Result<(), RepositoryStoreWriteError> {
    for record in &build.nodes {
        sink.write_all(&record.encode())?;
    }
    Ok(())
}

pub fn write_edges(
    sink: &mut SectionSink<'_>,
    build: &CompactRepositoryBuild,
) -> Result<(), RepositoryStoreWriteError> {
    for record in &build.edges {
        sink.write_all(&record.encode())?;
    }
    Ok(())
}

pub fn write_unresolved(
    sink: &mut SectionSink<'_>,
    build: &CompactRepositoryBuild,
) -> Result<(), RepositoryStoreWriteError> {
    for record in &build.unresolved {
        sink.write_all(&record.encode())?;
    }
    Ok(())
}

pub fn write_ownership(
    sink: &mut SectionSink<'_>,
    build: &CompactRepositoryBuild,
) -> Result<(), RepositoryStoreWriteError> {
    for record in &build.ownership {
        let mut bytes = [0_u8; 24];
        put_u32(&mut bytes, OWNERSHIP_PATH_ID_OFFSET, record.path.0);
        put_u64(
            &mut bytes,
            OWNERSHIP_CONTRIBUTION_START_OFFSET,
            record.contribution_start,
        );
        put_u64(
            &mut bytes,
            OWNERSHIP_CONTRIBUTION_COUNT_OFFSET,
            record.contribution_count,
        );
        sink.write_all(&bytes)?;
    }
    for contribution in &build.contributions {
        let mut bytes = [0_u8; 16];
        put_u64(
            &mut bytes,
            CONTRIBUTION_RECORD_INDEX_OFFSET,
            contribution.record_index,
        );
        bytes[CONTRIBUTION_KIND_OFFSET] = contribution.kind as u8;
        sink.write_all(&bytes)?;
    }
    Ok(())
}

pub fn write_name_index(
    sink: &mut SectionSink<'_>,
    build: &CompactRepositoryBuild,
) -> Result<(), RepositoryStoreWriteError> {
    write_dense_ids(sink, &build.name_index)
}

pub fn write_path_index(
    sink: &mut SectionSink<'_>,
    build: &CompactRepositoryBuild,
) -> Result<(), RepositoryStoreWriteError> {
    write_dense_ids(sink, &build.path_index)
}

pub fn write_kind_index(
    sink: &mut SectionSink<'_>,
    build: &CompactRepositoryBuild,
) -> Result<(), RepositoryStoreWriteError> {
    for record in &build.kind_index {
        let mut bytes = [0_u8; 8];
        put_u16(&mut bytes, KIND_INDEX_KIND_OFFSET, record.kind_code);
        put_u32(&mut bytes, KIND_INDEX_NODE_ID_OFFSET, record.node_id.0);
        sink.write_all(&bytes)?;
    }
    Ok(())
}

fn write_dense_ids(
    sink: &mut SectionSink<'_>,
    ids: &[crate::synthetic::NodeId],
) -> Result<(), RepositoryStoreWriteError> {
    for id in ids {
        sink.write_all(&id.0.to_le_bytes())?;
    }
    Ok(())
}

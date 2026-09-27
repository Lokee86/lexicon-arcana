use std::io::Write;

use crate::synthetic::NodeId;

use super::canonical::CanonicalFacts;
use super::format::{
    CONTRIBUTION_KIND_OFFSET, CONTRIBUTION_RECORD_INDEX_OFFSET, KIND_INDEX_KIND_OFFSET,
    KIND_INDEX_NODE_ID_OFFSET, OWNERSHIP_CONTRIBUTION_COUNT_OFFSET,
    OWNERSHIP_CONTRIBUTION_START_OFFSET, OWNERSHIP_PATH_ID_OFFSET,
};
use super::record_io::{put_u16, put_u32, put_u64};
use super::writer_sink::SectionSink;
use super::{
    BorrowedStringTable, CompactEdgeRecord, CompactNodeRecord, CompactUnresolvedRecord,
    RepositoryStoreWriteError, StringIdLookup,
};

pub fn write_strings(
    sink: &mut SectionSink<'_>,
    strings: &BorrowedStringTable<'_>,
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
    canonical: &CanonicalFacts<'_>,
    strings: &BorrowedStringTable<'_>,
) -> Result<(), RepositoryStoreWriteError> {
    for node in &canonical.nodes {
        let record = CompactNodeRecord::from_fact(node.fact, strings, node.occurrence_count)?;
        sink.write_all(&record.encode())?;
    }
    Ok(())
}

pub fn write_edges(
    sink: &mut SectionSink<'_>,
    canonical: &CanonicalFacts<'_>,
    strings: &BorrowedStringTable<'_>,
) -> Result<(), RepositoryStoreWriteError> {
    for edge in &canonical.edges {
        sink.write_all(&CompactEdgeRecord::from_fact(edge, strings)?.encode())?;
    }
    Ok(())
}

pub fn write_unresolved(
    sink: &mut SectionSink<'_>,
    canonical: &CanonicalFacts<'_>,
    strings: &BorrowedStringTable<'_>,
) -> Result<(), RepositoryStoreWriteError> {
    for reference in &canonical.unresolved {
        sink.write_all(&CompactUnresolvedRecord::from_fact(reference, strings)?.encode())?;
    }
    Ok(())
}

pub fn write_ownership(
    sink: &mut SectionSink<'_>,
    canonical: &CanonicalFacts<'_>,
    strings: &BorrowedStringTable<'_>,
) -> Result<(), RepositoryStoreWriteError> {
    let mut contribution_start = 0_u64;
    for (path, contributions) in &canonical.ownership {
        let mut record = [0_u8; 24];
        put_u32(&mut record, OWNERSHIP_PATH_ID_OFFSET, strings.id(path)?.0);
        put_u64(
            &mut record,
            OWNERSHIP_CONTRIBUTION_START_OFFSET,
            contribution_start,
        );
        let count = u64::try_from(contributions.len())
            .map_err(|_| RepositoryStoreWriteError::TooManyContributions)?;
        put_u64(&mut record, OWNERSHIP_CONTRIBUTION_COUNT_OFFSET, count);
        sink.write_all(&record)?;
        contribution_start = contribution_start
            .checked_add(count)
            .ok_or(RepositoryStoreWriteError::TooManyContributions)?;
    }
    for contributions in canonical.ownership.values() {
        for contribution in contributions {
            let mut record = [0_u8; 16];
            put_u64(
                &mut record,
                CONTRIBUTION_RECORD_INDEX_OFFSET,
                contribution.record_index,
            );
            record[CONTRIBUTION_KIND_OFFSET] = contribution.kind as u8;
            sink.write_all(&record)?;
        }
    }
    Ok(())
}

pub fn write_name_index(
    sink: &mut SectionSink<'_>,
    canonical: &CanonicalFacts<'_>,
) -> Result<(), RepositoryStoreWriteError> {
    let mut ids = dense_ids(canonical)?;
    ids.sort_unstable_by(|left, right| {
        canonical.nodes[left.0 as usize]
            .fact
            .name
            .cmp(&canonical.nodes[right.0 as usize].fact.name)
            .then_with(|| left.cmp(right))
    });
    write_dense_ids(sink, &ids)
}

pub fn write_path_index(
    sink: &mut SectionSink<'_>,
    canonical: &CanonicalFacts<'_>,
) -> Result<(), RepositoryStoreWriteError> {
    let mut ids = dense_ids(canonical)?;
    ids.sort_unstable_by(|left, right| {
        canonical.nodes[left.0 as usize]
            .fact
            .path
            .cmp(&canonical.nodes[right.0 as usize].fact.path)
            .then_with(|| left.cmp(right))
    });
    write_dense_ids(sink, &ids)
}

pub fn write_kind_index(
    sink: &mut SectionSink<'_>,
    canonical: &CanonicalFacts<'_>,
) -> Result<(), RepositoryStoreWriteError> {
    let mut ids = dense_ids(canonical)?;
    ids.sort_unstable_by_key(|id| {
        (
            super::format::node_kind_code(&canonical.nodes[id.0 as usize].fact.kind),
            *id,
        )
    });
    for id in ids {
        let mut record = [0_u8; 8];
        put_u16(
            &mut record,
            KIND_INDEX_KIND_OFFSET,
            super::format::node_kind_code(&canonical.nodes[id.0 as usize].fact.kind),
        );
        put_u32(&mut record, KIND_INDEX_NODE_ID_OFFSET, id.0);
        sink.write_all(&record)?;
    }
    Ok(())
}

fn dense_ids(canonical: &CanonicalFacts<'_>) -> Result<Vec<NodeId>, RepositoryStoreWriteError> {
    (0..canonical.nodes.len())
        .map(|index| {
            u32::try_from(index)
                .map(NodeId)
                .map_err(|_| RepositoryStoreWriteError::TooManyNodes)
        })
        .collect()
}

fn write_dense_ids(
    sink: &mut SectionSink<'_>,
    ids: &[NodeId],
) -> Result<(), RepositoryStoreWriteError> {
    for id in ids {
        sink.write_all(&id.0.to_le_bytes())?;
    }
    Ok(())
}

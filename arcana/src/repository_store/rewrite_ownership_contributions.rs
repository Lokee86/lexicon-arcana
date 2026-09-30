use std::collections::BTreeMap;
use std::io::{Read, Write};

use super::canonical::{Contribution, ContributionKind};
use super::format::{
    CONTRIBUTION_KIND_OFFSET, CONTRIBUTION_RECORD_INDEX_OFFSET, CONTRIBUTION_RECORD_LEN,
    FILE_OWNERSHIP_RECORD_LEN, OWNERSHIP_CONTRIBUTION_COUNT_OFFSET,
    OWNERSHIP_CONTRIBUTION_START_OFFSET, OWNERSHIP_PATH_ID_OFFSET,
};
use super::record_io::{get_u64, put_u32, put_u64};
use super::rewrite::ChangedRecords;
use super::rewrite_records::RecordStage;
use super::{RepositoryStoreWriteError, StringId};

pub(super) fn merge_owner_contributions(
    reader: &mut impl Read,
    base_count: u64,
    changed: &ChangedRecords,
    edges: &mut RecordStage,
    unresolved: &mut RecordStage,
    delta: &[Contribution],
    writer: &mut impl Write,
) -> Result<u64, RepositoryStoreWriteError> {
    let mut delta_index = 0_usize;
    let mut written = 0_u64;

    for _ in 0..base_count {
        let Some(base) = read_mapped_contribution(reader, changed, edges, unresolved)? else {
            continue;
        };
        while delta.get(delta_index).is_some_and(|value| *value < base) {
            write_contribution(writer, delta[delta_index])?;
            written += 1;
            delta_index += 1;
        }
        if delta.get(delta_index) == Some(&base) {
            delta_index += 1;
        }
        write_contribution(writer, base)?;
        written += 1;
    }

    while let Some(value) = delta.get(delta_index).copied() {
        write_contribution(writer, value)?;
        written += 1;
        delta_index += 1;
    }
    Ok(written)
}

pub(super) fn write_delta_contributions(
    writer: &mut impl Write,
    values: &[Contribution],
) -> Result<u64, RepositoryStoreWriteError> {
    for value in values {
        write_contribution(writer, *value)?;
    }
    Ok(values.len() as u64)
}

pub(super) fn write_owner_record(
    writer: &mut impl Write,
    path: StringId,
    start: u64,
    count: u64,
) -> Result<(), RepositoryStoreWriteError> {
    let mut bytes = [0_u8; FILE_OWNERSHIP_RECORD_LEN as usize];
    put_u32(&mut bytes, OWNERSHIP_PATH_ID_OFFSET, path.0);
    put_u64(&mut bytes, OWNERSHIP_CONTRIBUTION_START_OFFSET, start);
    put_u64(&mut bytes, OWNERSHIP_CONTRIBUTION_COUNT_OFFSET, count);
    writer.write_all(&bytes)?;
    Ok(())
}

pub(super) fn add(
    ownership: &mut BTreeMap<String, Vec<Contribution>>,
    path: String,
    kind: ContributionKind,
    record_index: u64,
) {
    ownership
        .entry(path)
        .or_default()
        .push(Contribution { kind, record_index });
}

pub(super) fn decode_map(encoded: u64) -> Result<u64, RepositoryStoreWriteError> {
    encoded
        .checked_sub(1)
        .ok_or(RepositoryStoreWriteError::MissingRewriteMapping(
            "delta-record",
        ))
}

fn read_mapped_contribution(
    reader: &mut impl Read,
    changed: &ChangedRecords,
    edges: &mut RecordStage,
    unresolved: &mut RecordStage,
) -> Result<Option<Contribution>, RepositoryStoreWriteError> {
    let mut bytes = [0_u8; CONTRIBUTION_RECORD_LEN as usize];
    reader.read_exact(&mut bytes)?;
    let index = get_u64(&bytes, CONTRIBUTION_RECORD_INDEX_OFFSET);
    let kind = match bytes[CONTRIBUTION_KIND_OFFSET] {
        1 => ContributionKind::Node,
        2 => ContributionKind::Edge,
        3 => ContributionKind::Unresolved,
        _ => {
            return Err(RepositoryStoreWriteError::MissingRewriteMapping(
                "ownership-kind",
            ));
        }
    };
    let mapped = match kind {
        ContributionKind::Node => {
            if changed.nodes.contains(&index) {
                return Ok(None);
            }
            index
        }
        ContributionKind::Edge => {
            let encoded = edges.base_map.get_u64(index)?;
            if encoded == 0 {
                return Ok(None);
            }
            encoded - 1
        }
        ContributionKind::Unresolved => {
            let encoded = unresolved.base_map.get_u64(index)?;
            if encoded == 0 {
                return Ok(None);
            }
            encoded - 1
        }
    };
    Ok(Some(Contribution {
        kind,
        record_index: mapped,
    }))
}

fn write_contribution(
    writer: &mut impl Write,
    contribution: Contribution,
) -> Result<(), RepositoryStoreWriteError> {
    let mut bytes = [0_u8; CONTRIBUTION_RECORD_LEN as usize];
    put_u64(
        &mut bytes,
        CONTRIBUTION_RECORD_INDEX_OFFSET,
        contribution.record_index,
    );
    bytes[CONTRIBUTION_KIND_OFFSET] = contribution.kind as u8;
    writer.write_all(&bytes)?;
    Ok(())
}

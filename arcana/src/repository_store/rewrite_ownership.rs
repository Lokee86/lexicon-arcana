use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Seek, SeekFrom, Write};
use std::path::PathBuf;

use super::build_ownership::compact_node_owner;
use super::canonical::{Contribution, ContributionKind};
use super::format::{
    FILE_OWNERSHIP_RECORD_LEN, OWNERSHIP_CONTRIBUTION_COUNT_OFFSET,
    OWNERSHIP_CONTRIBUTION_START_OFFSET, OWNERSHIP_PATH_ID_OFFSET, SectionKind,
};
use super::record_io::{get_u32, get_u64};
use super::rewrite::ChangedRecords;
use super::rewrite_io::{FixedSectionReader, RewriteWorkspace};
use super::rewrite_ownership_contributions::{
    add, decode_map, merge_owner_contributions, write_delta_contributions, write_owner_record,
};
use super::rewrite_records::{NodeStage, RecordStage};
use super::rewrite_strings::RewriteStrings;
use super::{CompactRepositoryDelta, RepositoryStoreFile, RepositoryStoreWriteError, StringId};

pub(super) struct OwnershipStage {
    pub(super) records_path: PathBuf,
    pub(super) contributions_path: PathBuf,
    pub(super) count: u64,
}

pub(super) fn stage_ownership(
    base: &mut RepositoryStoreFile,
    delta: &CompactRepositoryDelta,
    changed: &ChangedRecords,
    strings: &mut RewriteStrings,
    nodes: &NodeStage,
    edges: &mut RecordStage,
    unresolved: &mut RecordStage,
    work: &RewriteWorkspace,
) -> Result<OwnershipStage, RepositoryStoreWriteError> {
    let mut delta_ownership = build_delta_ownership(base, delta, nodes, edges, unresolved)?;

    let records_path = work.path("ownership-records.bin");
    let contributions_path = work.path("ownership-contributions.bin");
    let mut records_writer = BufWriter::new(File::create(&records_path)?);
    let mut contributions_writer = BufWriter::new(File::create(&contributions_path)?);
    let mut owner_reader = FixedSectionReader::<{ FILE_OWNERSHIP_RECORD_LEN as usize }>::open(
        base,
        SectionKind::Ownership,
    )?;
    let ownership = base.header.section(SectionKind::Ownership);
    let contribution_base = ownership
        .offset
        .checked_add(
            ownership
                .record_count
                .checked_mul(FILE_OWNERSHIP_RECORD_LEN)
                .ok_or(RepositoryStoreWriteError::SizeOverflow)?,
        )
        .ok_or(RepositoryStoreWriteError::SizeOverflow)?;
    let mut contribution_file = File::open(base.path())?;
    contribution_file.seek(SeekFrom::Start(contribution_base))?;
    let mut contribution_reader = BufReader::new(contribution_file);
    let mut base_contribution_position = 0_u64;
    let mut contribution_start = 0_u64;
    let mut owner_count = 0_u64;

    while let Some((_, bytes)) = owner_reader.next()? {
        let path_id = StringId(get_u32(&bytes, OWNERSHIP_PATH_ID_OFFSET));
        let path = base.string(path_id)?;
        let expected_start = get_u64(&bytes, OWNERSHIP_CONTRIBUTION_START_OFFSET);
        let base_count = get_u64(&bytes, OWNERSHIP_CONTRIBUTION_COUNT_OFFSET);
        if expected_start != base_contribution_position {
            return Err(RepositoryStoreWriteError::MissingRewriteMapping(
                "ownership-start",
            ));
        }

        while let Some((delta_path, _)) = delta_ownership.first_key_value() {
            if delta_path.as_str() >= path.as_str() {
                break;
            }
            let (delta_path, values) = delta_ownership.pop_first().expect("first key");
            let path_id = strings.remap_delta(delta.strings().id(&delta_path)?)?;
            let count = write_delta_contributions(&mut contributions_writer, &values)?;
            if count != 0 {
                write_owner_record(&mut records_writer, path_id, contribution_start, count)?;
                owner_count += 1;
                contribution_start += count;
            }
        }

        let delta_values = delta_ownership.remove(&path);
        let count = merge_owner_contributions(
            &mut contribution_reader,
            base_count,
            changed,
            edges,
            unresolved,
            delta_values.as_deref().unwrap_or(&[]),
            &mut contributions_writer,
        )?;
        base_contribution_position += base_count;
        if count != 0 {
            let final_path_id = strings.remap_value(delta, path_id, &path)?;
            write_owner_record(
                &mut records_writer,
                final_path_id,
                contribution_start,
                count,
            )?;
            owner_count += 1;
            contribution_start += count;
        }
    }

    for (path, values) in delta_ownership {
        let path_id = strings.remap_delta(delta.strings().id(&path)?)?;
        let count = write_delta_contributions(&mut contributions_writer, &values)?;
        if count != 0 {
            write_owner_record(&mut records_writer, path_id, contribution_start, count)?;
            owner_count += 1;
            contribution_start += count;
        }
    }

    records_writer.flush()?;
    contributions_writer.flush()?;
    Ok(OwnershipStage {
        records_path,
        contributions_path,
        count: owner_count,
    })
}

fn build_delta_ownership(
    base: &mut RepositoryStoreFile,
    delta: &CompactRepositoryDelta,
    nodes: &NodeStage,
    edges: &RecordStage,
    unresolved: &RecordStage,
) -> Result<BTreeMap<String, Vec<Contribution>>, RepositoryStoreWriteError> {
    let mut node_owners = BTreeMap::new();
    let mut ownership = BTreeMap::<String, Vec<Contribution>>::new();

    for (index, node) in delta.nodes().iter().enumerate() {
        if let Some(path_id) = compact_node_owner(delta.strings(), node)? {
            let path = delta.strings().get(path_id)?.to_owned();
            node_owners.insert(node.key, path.clone());
            add(
                &mut ownership,
                path,
                ContributionKind::Node,
                u64::from(nodes.delta_node_ids[index].0),
            );
        }
    }

    for (index, edge) in delta.edges().iter().enumerate() {
        let owner = if let Some(span) = edge.span {
            Some(delta.strings().get(span.path)?.to_owned())
        } else {
            node_owner(base, &node_owners, edge.source)?.or(node_owner(
                base,
                &node_owners,
                edge.target,
            )?)
        };
        if let Some(path) = owner {
            add(
                &mut ownership,
                path,
                ContributionKind::Edge,
                decode_map(edges.delta_map[index])?,
            );
        }
    }

    for (index, reference) in delta.unresolved().iter().enumerate() {
        let owner = if let Some(span) = reference.span {
            Some(delta.strings().get(span.path)?.to_owned())
        } else {
            node_owner(base, &node_owners, reference.source)?
        };
        if let Some(path) = owner {
            add(
                &mut ownership,
                path,
                ContributionKind::Unresolved,
                decode_map(unresolved.delta_map[index])?,
            );
        }
    }

    for values in ownership.values_mut() {
        values.sort_unstable();
        values.dedup();
    }
    Ok(ownership)
}

fn node_owner(
    base: &mut RepositoryStoreFile,
    delta: &BTreeMap<crate::repository::NodeKey, String>,
    key: crate::repository::NodeKey,
) -> Result<Option<String>, RepositoryStoreWriteError> {
    if let Some(path) = delta.get(&key) {
        return Ok(Some(path.clone()));
    }
    Ok(base.node_owner_path(key)?)
}

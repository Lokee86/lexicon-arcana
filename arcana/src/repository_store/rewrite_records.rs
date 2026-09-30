use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use crate::repository::{NodeKey, NodeKind};
use crate::storage::StableHasher;
use crate::synthetic::NodeId;

use super::build_indexes::{compare_edges, compare_unresolved};
use super::format::{EDGE_RECORD_LEN, NODE_RECORD_LEN, SectionKind, UNRESOLVED_RECORD_LEN};
use super::rewrite::ChangedRecords;
use super::rewrite_io::{FixedMapFile, FixedSectionReader, RewriteWorkspace};
use super::rewrite_record_remap::{
    next_base_edge, next_base_unresolved, next_delta_edge, next_delta_unresolved, remap_base_node,
    remap_delta_node,
};
use super::rewrite_strings::RewriteStrings;
use super::{
    CompactNodeRecord, CompactRepositoryDelta, RepositoryStoreFile, RepositoryStoreWriteError,
};

pub(super) struct NodeStage {
    pub(super) path: PathBuf,
    pub(super) count: u64,
    pub(super) node_key_checksum: u64,
    pub(super) repository_key: Option<NodeKey>,
    pub(super) delta_node_ids: Vec<NodeId>,
}

pub(super) struct RecordStage {
    pub(super) path: PathBuf,
    pub(super) count: u64,
    pub(super) base_map: FixedMapFile,
    pub(super) delta_map: Vec<u64>,
}

pub(super) fn stage_nodes(
    base: &RepositoryStoreFile,
    delta: &CompactRepositoryDelta,
    changed: &ChangedRecords,
    strings: &mut RewriteStrings,
    work: &RewriteWorkspace,
) -> Result<NodeStage, RepositoryStoreWriteError> {
    let path = work.path("nodes.bin");
    let mut writer = BufWriter::new(File::create(&path)?);
    let mut reader =
        FixedSectionReader::<{ NODE_RECORD_LEN as usize }>::open(base, SectionKind::Nodes)?;
    let mut delta_node_ids = vec![NodeId(u32::MAX); delta.nodes().len()];
    let mut hasher = StableHasher::new();
    let repository_kind = super::format::node_kind_code(&NodeKind::Repository);
    let mut repository_key = None;
    let mut repository_ambiguous = false;
    let mut count = 0_u64;

    while let Some((index, bytes)) = reader.next()? {
        let base_record = CompactNodeRecord::decode(&bytes)?;
        let record = if changed.nodes.contains(&index) {
            let delta_index = delta
                .nodes()
                .binary_search_by_key(&base_record.key, |record| record.key)
                .map_err(|_| RepositoryStoreWriteError::ReplacementNodeSetMismatch)?;
            let node_id = u32::try_from(index)
                .map(NodeId)
                .map_err(|_| RepositoryStoreWriteError::TooManyNodes)?;
            delta_node_ids[delta_index] = node_id;
            remap_delta_node(delta.nodes()[delta_index], strings)?
        } else {
            remap_base_node(base_record, strings)?
        };
        if record.kind_code == repository_kind {
            if repository_key.is_some() {
                repository_ambiguous = true;
            } else {
                repository_key = Some(record.key);
            }
        }
        hasher.update(&record.key.0.to_le_bytes());
        writer.write_all(&record.encode())?;
        count += 1;
    }
    writer.flush()?;

    if delta_node_ids.iter().any(|id| id.0 == u32::MAX) {
        return Err(RepositoryStoreWriteError::ReplacementNodeSetMismatch);
    }

    Ok(NodeStage {
        path,
        count,
        node_key_checksum: hasher.finish(),
        repository_key: (!repository_ambiguous).then_some(repository_key).flatten(),
        delta_node_ids,
    })
}

pub(super) fn stage_edges(
    base: &RepositoryStoreFile,
    delta: &CompactRepositoryDelta,
    changed: &ChangedRecords,
    strings: &mut RewriteStrings,
    work: &RewriteWorkspace,
) -> Result<RecordStage, RepositoryStoreWriteError> {
    let path = work.path("edges.bin");
    let mut writer = BufWriter::new(File::create(&path)?);
    let base_count = base.header.section(SectionKind::Edges).record_count;
    let mut base_map = FixedMapFile::create(&work.path("base-edge-map.bin"), base_count, 8)?;
    let mut delta_map = vec![0_u64; delta.edges().len()];
    let mut reader =
        FixedSectionReader::<{ EDGE_RECORD_LEN as usize }>::open(base, SectionKind::Edges)?;
    let mut delta_index = 0_usize;
    let mut base_next = next_base_edge(&mut reader, &changed.edges, strings)?;
    let mut delta_next = next_delta_edge(delta, delta_index, strings)?;
    let mut final_index = 0_u64;

    while base_next.is_some() || delta_next.is_some() {
        match (&base_next, &delta_next) {
            (Some((base_index, base_record)), Some(delta_record)) => {
                match compare_edges(base_record, delta_record) {
                    std::cmp::Ordering::Less => {
                        writer.write_all(&base_record.encode())?;
                        base_map.set_u64(*base_index, final_index + 1)?;
                        base_next = next_base_edge(&mut reader, &changed.edges, strings)?;
                    }
                    std::cmp::Ordering::Greater => {
                        writer.write_all(&delta_record.encode())?;
                        delta_map[delta_index] = final_index + 1;
                        delta_index += 1;
                        delta_next = next_delta_edge(delta, delta_index, strings)?;
                    }
                    std::cmp::Ordering::Equal => {
                        writer.write_all(&base_record.encode())?;
                        base_map.set_u64(*base_index, final_index + 1)?;
                        delta_map[delta_index] = final_index + 1;
                        base_next = next_base_edge(&mut reader, &changed.edges, strings)?;
                        delta_index += 1;
                        delta_next = next_delta_edge(delta, delta_index, strings)?;
                    }
                }
            }
            (Some((base_index, base_record)), None) => {
                writer.write_all(&base_record.encode())?;
                base_map.set_u64(*base_index, final_index + 1)?;
                base_next = next_base_edge(&mut reader, &changed.edges, strings)?;
            }
            (None, Some(delta_record)) => {
                writer.write_all(&delta_record.encode())?;
                delta_map[delta_index] = final_index + 1;
                delta_index += 1;
                delta_next = next_delta_edge(delta, delta_index, strings)?;
            }
            (None, None) => break,
        }
        final_index += 1;
    }
    writer.flush()?;

    Ok(RecordStage {
        path,
        count: final_index,
        base_map,
        delta_map,
    })
}

pub(super) fn stage_unresolved(
    base: &RepositoryStoreFile,
    delta: &CompactRepositoryDelta,
    changed: &ChangedRecords,
    strings: &mut RewriteStrings,
    work: &RewriteWorkspace,
) -> Result<RecordStage, RepositoryStoreWriteError> {
    let path = work.path("unresolved.bin");
    let mut writer = BufWriter::new(File::create(&path)?);
    let base_count = base.header.section(SectionKind::Unresolved).record_count;
    let mut base_map = FixedMapFile::create(&work.path("base-unresolved-map.bin"), base_count, 8)?;
    let mut delta_map = vec![0_u64; delta.unresolved().len()];
    let mut reader = FixedSectionReader::<{ UNRESOLVED_RECORD_LEN as usize }>::open(
        base,
        SectionKind::Unresolved,
    )?;
    let mut delta_index = 0_usize;
    let mut base_next = next_base_unresolved(&mut reader, &changed.unresolved, strings)?;
    let mut delta_next = next_delta_unresolved(delta, delta_index, strings)?;
    let mut final_index = 0_u64;

    while base_next.is_some() || delta_next.is_some() {
        match (&base_next, &delta_next) {
            (Some((base_index, base_record)), Some(delta_record)) => {
                match compare_unresolved(base_record, delta_record) {
                    std::cmp::Ordering::Less => {
                        writer.write_all(&base_record.encode())?;
                        base_map.set_u64(*base_index, final_index + 1)?;
                        base_next =
                            next_base_unresolved(&mut reader, &changed.unresolved, strings)?;
                    }
                    std::cmp::Ordering::Greater => {
                        writer.write_all(&delta_record.encode())?;
                        delta_map[delta_index] = final_index + 1;
                        delta_index += 1;
                        delta_next = next_delta_unresolved(delta, delta_index, strings)?;
                    }
                    std::cmp::Ordering::Equal => {
                        writer.write_all(&base_record.encode())?;
                        base_map.set_u64(*base_index, final_index + 1)?;
                        delta_map[delta_index] = final_index + 1;
                        base_next =
                            next_base_unresolved(&mut reader, &changed.unresolved, strings)?;
                        delta_index += 1;
                        delta_next = next_delta_unresolved(delta, delta_index, strings)?;
                    }
                }
            }
            (Some((base_index, base_record)), None) => {
                writer.write_all(&base_record.encode())?;
                base_map.set_u64(*base_index, final_index + 1)?;
                base_next = next_base_unresolved(&mut reader, &changed.unresolved, strings)?;
            }
            (None, Some(delta_record)) => {
                writer.write_all(&delta_record.encode())?;
                delta_map[delta_index] = final_index + 1;
                delta_index += 1;
                delta_next = next_delta_unresolved(delta, delta_index, strings)?;
            }
            (None, None) => break,
        }
        final_index += 1;
    }
    writer.flush()?;

    Ok(RecordStage {
        path,
        count: final_index,
        base_map,
        delta_map,
    })
}

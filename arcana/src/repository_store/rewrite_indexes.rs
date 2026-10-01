use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use crate::synthetic::NodeId;

use super::format::{
    KIND_INDEX_KIND_OFFSET, KIND_INDEX_NODE_ID_OFFSET, KIND_INDEX_RECORD_LEN,
    NAME_INDEX_RECORD_LEN, PATH_INDEX_RECORD_LEN, SectionKind,
};
use super::record_io::{get_u16, get_u32, put_u16, put_u32};
use super::rewrite::ChangedRecords;
use super::rewrite_io::{FixedSectionReader, RewriteWorkspace};
use super::rewrite_records::NodeStage;
use super::rewrite_strings::RewriteStrings;
use super::{CompactRepositoryDelta, RepositoryStoreFile, RepositoryStoreWriteError, StringId};

pub(super) struct IndexStage {
    pub(super) name_path: PathBuf,
    pub(super) path_path: PathBuf,
    pub(super) kind_path: PathBuf,
    pub(super) count: u64,
}

pub(super) fn stage_indexes(
    base: &mut RepositoryStoreFile,
    delta: &CompactRepositoryDelta,
    changed: &ChangedRecords,
    strings: &mut RewriteStrings,
    nodes: &NodeStage,
    work: &RewriteWorkspace,
) -> Result<IndexStage, RepositoryStoreWriteError> {
    let name_path = work.path("name-index.bin");
    let path_path = work.path("path-index.bin");
    let kind_path = work.path("kind-index.bin");

    stage_string_index(
        base,
        delta,
        changed,
        strings,
        nodes,
        SectionKind::NameIndex,
        &name_path,
        |node| node.name,
    )?;
    stage_string_index(
        base,
        delta,
        changed,
        strings,
        nodes,
        SectionKind::PathIndex,
        &path_path,
        |node| node.path,
    )?;
    stage_kind_index(base, delta, changed, nodes, &kind_path)?;

    Ok(IndexStage {
        name_path,
        path_path,
        kind_path,
        count: nodes.count,
    })
}

fn stage_string_index(
    base: &mut RepositoryStoreFile,
    delta: &CompactRepositoryDelta,
    changed: &ChangedRecords,
    strings: &mut RewriteStrings,
    nodes: &NodeStage,
    kind: SectionKind,
    output: &std::path::Path,
    key: impl Fn(&super::CompactNodeRecord) -> StringId,
) -> Result<(), RepositoryStoreWriteError> {
    let mut delta_entries = delta
        .nodes()
        .iter()
        .enumerate()
        .map(|(index, node)| Ok((strings.remap_delta(key(node))?, nodes.delta_node_ids[index])))
        .collect::<Result<Vec<_>, RepositoryStoreWriteError>>()?;
    delta_entries.sort_unstable();

    let mut reader = match kind {
        SectionKind::NameIndex => {
            FixedSectionReader::<{ NAME_INDEX_RECORD_LEN as usize }>::open(base, kind)?
        }
        SectionKind::PathIndex => {
            FixedSectionReader::<{ PATH_INDEX_RECORD_LEN as usize }>::open(base, kind)?
        }
        _ => unreachable!("string index section"),
    };
    let mut writer = BufWriter::new(File::create(output)?);
    let mut delta_index = 0_usize;
    let mut base_next = next_base_string_index(base, changed, strings, &mut reader, &key)?;
    let mut written = 0_u64;

    while base_next.is_some() || delta_index < delta_entries.len() {
        let delta_next = delta_entries.get(delta_index).copied();
        match (base_next, delta_next) {
            (Some(base_entry), Some(delta_entry)) if base_entry <= delta_entry => {
                write_node_id(&mut writer, base_entry.1)?;
                base_next = next_base_string_index(base, changed, strings, &mut reader, &key)?;
            }
            (_, Some(delta_entry)) => {
                write_node_id(&mut writer, delta_entry.1)?;
                delta_index += 1;
            }
            (Some(base_entry), None) => {
                write_node_id(&mut writer, base_entry.1)?;
                base_next = next_base_string_index(base, changed, strings, &mut reader, &key)?;
            }
            (None, None) => break,
        }
        written += 1;
    }
    writer.flush()?;

    if written != nodes.count {
        return Err(RepositoryStoreWriteError::MissingRewriteMapping(
            "string-index-count",
        ));
    }
    Ok(())
}

fn next_base_string_index<const N: usize>(
    base: &mut RepositoryStoreFile,
    changed: &ChangedRecords,
    strings: &mut RewriteStrings,
    reader: &mut FixedSectionReader<N>,
    key: &impl Fn(&super::CompactNodeRecord) -> StringId,
) -> Result<Option<(StringId, NodeId)>, RepositoryStoreWriteError> {
    while let Some((_, bytes)) = reader.next()? {
        let node_id = NodeId(get_u32(&bytes, 0));
        if changed.nodes.contains(&u64::from(node_id.0)) {
            continue;
        }
        let node = base.node_record(node_id.0)?;
        return Ok(Some((strings.remap_base(key(&node))?, node_id)));
    }
    Ok(None)
}

fn stage_kind_index(
    base: &RepositoryStoreFile,
    delta: &CompactRepositoryDelta,
    changed: &ChangedRecords,
    nodes: &NodeStage,
    output: &std::path::Path,
) -> Result<(), RepositoryStoreWriteError> {
    let mut delta_entries = delta
        .nodes()
        .iter()
        .enumerate()
        .map(|(index, node)| (node.kind_code, nodes.delta_node_ids[index]))
        .collect::<Vec<_>>();
    delta_entries.sort_unstable();

    let mut reader = FixedSectionReader::<{ KIND_INDEX_RECORD_LEN as usize }>::open(
        base,
        SectionKind::KindIndex,
    )?;
    let mut writer = BufWriter::new(File::create(output)?);
    let mut delta_index = 0_usize;
    let mut base_next = next_base_kind_index(changed, &mut reader)?;
    let mut written = 0_u64;

    while base_next.is_some() || delta_index < delta_entries.len() {
        let delta_next = delta_entries.get(delta_index).copied();
        match (base_next, delta_next) {
            (Some(base_entry), Some(delta_entry)) if base_entry <= delta_entry => {
                write_kind(&mut writer, base_entry)?;
                base_next = next_base_kind_index(changed, &mut reader)?;
            }
            (_, Some(delta_entry)) => {
                write_kind(&mut writer, delta_entry)?;
                delta_index += 1;
            }
            (Some(base_entry), None) => {
                write_kind(&mut writer, base_entry)?;
                base_next = next_base_kind_index(changed, &mut reader)?;
            }
            (None, None) => break,
        }
        written += 1;
    }
    writer.flush()?;

    if written != nodes.count {
        return Err(RepositoryStoreWriteError::MissingRewriteMapping(
            "kind-index-count",
        ));
    }
    Ok(())
}

fn next_base_kind_index(
    changed: &ChangedRecords,
    reader: &mut FixedSectionReader<{ KIND_INDEX_RECORD_LEN as usize }>,
) -> Result<Option<(u16, NodeId)>, RepositoryStoreWriteError> {
    while let Some((_, bytes)) = reader.next()? {
        let node_id = NodeId(get_u32(&bytes, KIND_INDEX_NODE_ID_OFFSET));
        if changed.nodes.contains(&u64::from(node_id.0)) {
            continue;
        }
        return Ok(Some((get_u16(&bytes, KIND_INDEX_KIND_OFFSET), node_id)));
    }
    Ok(None)
}

fn write_node_id(
    writer: &mut impl Write,
    node_id: NodeId,
) -> Result<(), RepositoryStoreWriteError> {
    writer.write_all(&node_id.0.to_le_bytes())?;
    Ok(())
}

fn write_kind(
    writer: &mut impl Write,
    (kind, node_id): (u16, NodeId),
) -> Result<(), RepositoryStoreWriteError> {
    let mut bytes = [0_u8; KIND_INDEX_RECORD_LEN as usize];
    put_u16(&mut bytes, KIND_INDEX_KIND_OFFSET, kind);
    put_u32(&mut bytes, KIND_INDEX_NODE_ID_OFFSET, node_id.0);
    writer.write_all(&bytes)?;
    Ok(())
}

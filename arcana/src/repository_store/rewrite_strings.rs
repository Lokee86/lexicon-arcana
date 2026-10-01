use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;

use super::format::{
    EDGE_RECORD_LEN, NODE_RECORD_LEN, STRING_INDEX_RECORD_LEN, SectionKind, UNRESOLVED_RECORD_LEN,
};
use super::record_io::{put_u32, put_u64};
use super::rewrite::ChangedRecords;
use super::rewrite_io::{FixedMapFile, FixedSectionReader, RewriteWorkspace, copy_file};
use super::rewrite_sort::{read_u32, sort_unique_u32};
use super::{
    CompactEdgeRecord, CompactNodeRecord, CompactRepositoryDelta, CompactUnresolvedRecord,
    RepositoryStoreFile, RepositoryStoreWriteError, StringId,
};

pub(super) struct RewriteStrings {
    index_path: PathBuf,
    blob_path: PathBuf,
    pub(super) count: u64,
    base_map: FixedMapFile,
    delta_map: Vec<u32>,
}

impl RewriteStrings {
    pub(super) fn build(
        base: &mut RepositoryStoreFile,
        delta: &CompactRepositoryDelta,
        changed: &ChangedRecords,
        work: &RewriteWorkspace,
    ) -> Result<Self, RepositoryStoreWriteError> {
        let raw = work.path("used-string-ids.raw");
        collect_used_base_strings(base, changed, &raw)?;
        let used = work.path("used-string-ids.sorted");
        sort_unique_u32(&raw, &used, &work.path("sort"))?;

        let base_string_count = base.header.section(SectionKind::Strings).record_count;
        let mut base_map =
            FixedMapFile::create(&work.path("base-string-map.bin"), base_string_count, 4)?;
        let mut delta_map = vec![u32::MAX; delta.strings().len()];
        let index_path = work.path("strings-index.bin");
        let blob_path = work.path("strings-blob.bin");
        let mut index_writer = BufWriter::new(File::create(&index_path)?);
        let mut blob_writer = BufWriter::new(File::create(&blob_path)?);

        let mut used_reader = BufReader::new(File::open(&used)?);
        let mut base_id = read_u32(&mut used_reader)?;
        let mut delta_id = 0_usize;
        let mut blob_offset = 0_u64;
        let mut final_id = 0_u32;

        while base_id.is_some() || delta_id < delta.strings().len() {
            let base_value = base_id.map(|id| base.string(StringId(id))).transpose()?;
            let delta_value = if delta_id < delta.strings().len() {
                Some(delta.strings().get(StringId(delta_id as u32))?)
            } else {
                None
            };

            let take = match (base_value.as_deref(), delta_value) {
                (Some(left), Some(right)) => left.cmp(right),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => break,
            };

            let value = match take {
                std::cmp::Ordering::Less => base_value.as_deref().expect("base value"),
                std::cmp::Ordering::Greater => delta_value.expect("delta value"),
                std::cmp::Ordering::Equal => base_value.as_deref().expect("equal value"),
            };
            write_string_entry(&mut index_writer, &mut blob_writer, blob_offset, value)?;
            let encoded = final_id
                .checked_add(1)
                .ok_or(RepositoryStoreWriteError::SizeOverflow)?;

            if take != std::cmp::Ordering::Greater {
                let id = base_id.expect("base id");
                base_map.set_u32(u64::from(id), encoded)?;
                base_id = read_u32(&mut used_reader)?;
            }
            if take != std::cmp::Ordering::Less {
                delta_map[delta_id] = final_id;
                delta_id += 1;
            }

            blob_offset = blob_offset
                .checked_add(value.len() as u64)
                .ok_or(RepositoryStoreWriteError::SizeOverflow)?;
            final_id = final_id
                .checked_add(1)
                .ok_or(RepositoryStoreWriteError::SizeOverflow)?;
            if final_id == StringId::ABSENT.0 {
                return Err(super::StoreFormatError::TooManyStrings.into());
            }
        }

        index_writer.flush()?;
        blob_writer.flush()?;
        Ok(Self {
            index_path,
            blob_path,
            count: u64::from(final_id),
            base_map,
            delta_map,
        })
    }

    pub(super) fn remap_base(
        &mut self,
        id: StringId,
    ) -> Result<StringId, RepositoryStoreWriteError> {
        if id == StringId::ABSENT {
            return Ok(id);
        }
        let encoded = self.base_map.get_u32(u64::from(id.0))?;
        if encoded == 0 {
            return Err(RepositoryStoreWriteError::MissingRewriteMapping(
                "base-string",
            ));
        }
        Ok(StringId(encoded - 1))
    }

    pub(super) fn remap_delta(&self, id: StringId) -> Result<StringId, RepositoryStoreWriteError> {
        if id == StringId::ABSENT {
            return Ok(id);
        }
        let mapped = self
            .delta_map
            .get(id.0 as usize)
            .copied()
            .filter(|mapped| *mapped != u32::MAX)
            .ok_or(RepositoryStoreWriteError::MissingRewriteMapping(
                "delta-string",
            ))?;
        Ok(StringId(mapped))
    }

    pub(super) fn remap_value(
        &mut self,
        delta: &CompactRepositoryDelta,
        base_id: StringId,
        value: &str,
    ) -> Result<StringId, RepositoryStoreWriteError> {
        match delta.strings().id(value) {
            Ok(delta_id) => self.remap_delta(delta_id),
            Err(super::StoreFormatError::MissingString) => self.remap_base(base_id),
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn write_to(&self, sink: &mut impl Write) -> Result<(), RepositoryStoreWriteError> {
        copy_file(&self.index_path, sink)?;
        copy_file(&self.blob_path, sink)
    }
}

fn collect_used_base_strings(
    base: &RepositoryStoreFile,
    changed: &ChangedRecords,
    path: &std::path::Path,
) -> Result<(), RepositoryStoreWriteError> {
    let mut writer = BufWriter::new(File::create(path)?);

    let mut nodes =
        FixedSectionReader::<{ NODE_RECORD_LEN as usize }>::open(base, SectionKind::Nodes)?;
    while let Some((index, bytes)) = nodes.next()? {
        if changed.nodes.contains(&index) {
            continue;
        }
        let record = CompactNodeRecord::decode(&bytes)?;
        for id in [record.path, record.name, record.qualified_name] {
            write_id(&mut writer, id)?;
        }
        if let Some(span) = record.span {
            write_id(&mut writer, span.path)?;
        }
    }

    let mut edges =
        FixedSectionReader::<{ EDGE_RECORD_LEN as usize }>::open(base, SectionKind::Edges)?;
    while let Some((index, bytes)) = edges.next()? {
        if changed.edges.contains(&index) {
            continue;
        }
        let record = CompactEdgeRecord::decode(&bytes)?;
        if let Some(span) = record.span {
            write_id(&mut writer, span.path)?;
        }
    }

    let mut unresolved = FixedSectionReader::<{ UNRESOLVED_RECORD_LEN as usize }>::open(
        base,
        SectionKind::Unresolved,
    )?;
    while let Some((index, bytes)) = unresolved.next()? {
        if changed.unresolved.contains(&index) {
            continue;
        }
        let record = CompactUnresolvedRecord::decode(&bytes)?;
        for id in [
            record.expression,
            record.candidate_namespace,
            record.candidate_name,
            record.unknown_reason,
        ] {
            write_id(&mut writer, id)?;
        }
        if let Some(span) = record.span {
            write_id(&mut writer, span.path)?;
        }
    }

    writer.flush()?;
    Ok(())
}

fn write_id(writer: &mut impl Write, id: StringId) -> Result<(), RepositoryStoreWriteError> {
    if id != StringId::ABSENT {
        writer.write_all(&id.0.to_le_bytes())?;
    }
    Ok(())
}

fn write_string_entry(
    index: &mut impl Write,
    blob: &mut impl Write,
    offset: u64,
    value: &str,
) -> Result<(), RepositoryStoreWriteError> {
    let len = u32::try_from(value.len()).map_err(|_| super::StoreFormatError::StringTooLong)?;
    let mut record = [0_u8; STRING_INDEX_RECORD_LEN as usize];
    put_u64(&mut record, 0, offset);
    put_u32(&mut record, 8, len);
    index.write_all(&record)?;
    blob.write_all(value.as_bytes())?;
    Ok(())
}

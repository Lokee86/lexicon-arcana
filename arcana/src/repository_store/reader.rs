use std::fs;
use std::path::Path;

use super::format::{
    EDGE_RECORD_LEN, NODE_RECORD_LEN, RepositoryHeader, SectionKind, UNRESOLVED_RECORD_LEN,
};
use super::{
    CompactEdgeRecord, CompactNodeRecord, CompactUnresolvedRecord, EdgeRecordView, NodeRecordView,
    OwnershipView, RepositoryStoreReadError, StringTableView, UnresolvedRecordView,
};

pub struct RepositoryStore {
    pub(super) bytes: Box<[u8]>,
    pub(super) header: RepositoryHeader,
}

impl RepositoryStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RepositoryStoreReadError> {
        Self::from_bytes(fs::read(path)?.into_boxed_slice())
    }

    pub fn from_bytes(bytes: Box<[u8]>) -> Result<Self, RepositoryStoreReadError> {
        let header = RepositoryHeader::decode(&bytes)?;
        let actual = bytes.len() as u64;
        if actual != header.file_len {
            return Err(RepositoryStoreReadError::FileLength {
                expected: header.file_len,
                actual,
            });
        }
        super::reader_validation::verify_checksums(&bytes, &header)?;
        super::reader_validation::verify_padding(&bytes, &header)?;
        Ok(Self { bytes, header })
    }

    pub const fn header(&self) -> &RepositoryHeader {
        &self.header
    }

    pub fn node_count(&self) -> u32 {
        self.header.section(SectionKind::Nodes).record_count as u32
    }

    pub fn edge_count(&self) -> u64 {
        self.header.section(SectionKind::Edges).record_count
    }

    pub fn unresolved_count(&self) -> u64 {
        self.header.section(SectionKind::Unresolved).record_count
    }

    pub fn strings(&self) -> Result<StringTableView<'_>, RepositoryStoreReadError> {
        let descriptor = self.header.section(SectionKind::Strings);
        StringTableView::new(self.section(SectionKind::Strings), descriptor.record_count)
    }

    pub fn node(&self, node_id: u32) -> Result<NodeRecordView<'_>, RepositoryStoreReadError> {
        let bytes = record(
            self.section(SectionKind::Nodes),
            node_id as u64,
            NODE_RECORD_LEN,
        )
        .ok_or(RepositoryStoreReadError::InvalidNodeId(node_id))?;
        let compact = CompactNodeRecord::decode(bytes.try_into().expect("node record width"))?;
        Ok(NodeRecordView::new(node_id, compact, self.strings()?))
    }

    pub fn edge(&self, index: u64) -> Result<Option<EdgeRecordView<'_>>, RepositoryStoreReadError> {
        let Some(bytes) = record(self.section(SectionKind::Edges), index, EDGE_RECORD_LEN) else {
            return Ok(None);
        };
        let compact = CompactEdgeRecord::decode(bytes.try_into().expect("edge record width"))?;
        Ok(Some(EdgeRecordView::new(compact, self.strings()?)))
    }

    pub fn unresolved(
        &self,
        index: u64,
    ) -> Result<Option<UnresolvedRecordView<'_>>, RepositoryStoreReadError> {
        let Some(bytes) = record(
            self.section(SectionKind::Unresolved),
            index,
            UNRESOLVED_RECORD_LEN,
        ) else {
            return Ok(None);
        };
        let compact =
            CompactUnresolvedRecord::decode(bytes.try_into().expect("unresolved record width"))?;
        Ok(Some(UnresolvedRecordView::new(compact, self.strings()?)))
    }

    pub fn ownership(&self) -> Result<OwnershipView<'_>, RepositoryStoreReadError> {
        OwnershipView::new(
            self.section(SectionKind::Ownership),
            self.header.section(SectionKind::Ownership).record_count,
            self.strings()?,
            self.header.section(SectionKind::Nodes).record_count,
            self.header.section(SectionKind::Edges).record_count,
            self.header.section(SectionKind::Unresolved).record_count,
        )
    }

    pub(super) fn section(&self, kind: SectionKind) -> &[u8] {
        super::reader_validation::section(&self.bytes, &self.header, kind)
    }
}

fn record(bytes: &[u8], index: u64, width: u64) -> Option<&[u8]> {
    let start = usize::try_from(index.checked_mul(width)?).ok()?;
    let end = start.checked_add(width as usize)?;
    bytes.get(start..end)
}

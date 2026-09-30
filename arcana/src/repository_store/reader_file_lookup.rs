use crate::repository::NodeKey;

use super::format::{
    CONTRIBUTION_KIND_OFFSET, CONTRIBUTION_RECORD_INDEX_OFFSET, CONTRIBUTION_RECORD_LEN,
    EDGE_RECORD_LEN, FILE_OWNERSHIP_RECORD_LEN, FormatError, NODE_KEY_OFFSET, NODE_RECORD_LEN,
    OWNERSHIP_CONTRIBUTION_COUNT_OFFSET, OWNERSHIP_CONTRIBUTION_START_OFFSET,
    OWNERSHIP_PATH_ID_OFFSET, STRING_ENTRY_LENGTH_OFFSET, STRING_ENTRY_OFFSET_OFFSET,
    STRING_INDEX_RECORD_LEN, SectionKind, UNRESOLVED_RECORD_LEN,
};
use super::record_io::{get_u32, get_u64};
use super::{
    CompactEdgeRecord, CompactNodeRecord, CompactUnresolvedRecord, ContributionKindView,
    RepositoryStoreFile, RepositoryStoreReadError, Sha256Identity, StoreFormatError, StringId,
};

impl RepositoryStoreFile {
    pub fn contains_node_key(&mut self, key: NodeKey) -> Result<bool, RepositoryStoreReadError> {
        Ok(self.find_node_record(key)?.is_some())
    }

    pub fn contains_node_identity(
        &mut self,
        key: NodeKey,
        identity: Sha256Identity,
    ) -> Result<bool, RepositoryStoreReadError> {
        Ok(self
            .find_node_record(key)?
            .is_some_and(|record| record.external_identity == Some(identity)))
    }

    fn find_node_record(
        &mut self,
        key: NodeKey,
    ) -> Result<Option<CompactNodeRecord>, RepositoryStoreReadError> {
        let count = self.header.section(SectionKind::Nodes).record_count;
        let mut low = 0_u64;
        let mut high = count;
        while low < high {
            let mid = low + (high - low) / 2;
            let node_id = u32::try_from(mid).map_err(|_| FormatError::TooManyNodes)?;
            let record = self.node_record(node_id)?;
            match record.key.cmp(&key) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => return Ok(Some(record)),
            }
        }
        Ok(None)
    }

    pub(super) fn find_ownership(
        &mut self,
        path: &str,
    ) -> Result<Option<u64>, RepositoryStoreReadError> {
        let count = self.header.section(SectionKind::Ownership).record_count;
        let mut low = 0_u64;
        let mut high = count;
        while low < high {
            let mid = low + (high - low) / 2;
            let record = self.ownership_record(mid)?;
            let id = StringId(get_u32(&record, OWNERSHIP_PATH_ID_OFFSET));
            match self.string(id)?.as_str().cmp(path) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => return Ok(Some(mid)),
            }
        }
        Ok(None)
    }

    pub(super) fn for_each_owned_contribution(
        &mut self,
        index: u64,
        mut visit: impl FnMut(ContributionKindView, u64) -> Result<(), RepositoryStoreReadError>,
    ) -> Result<(), RepositoryStoreReadError> {
        let record = self.ownership_record(index)?;
        let start = get_u64(&record, OWNERSHIP_CONTRIBUTION_START_OFFSET);
        let count = get_u64(&record, OWNERSHIP_CONTRIBUTION_COUNT_OFFSET);
        let contribution_count = self.contribution_count()?;
        let end = start
            .checked_add(count)
            .filter(|end| *end <= contribution_count)
            .ok_or(RepositoryStoreReadError::InvalidOwnership)?;

        let descriptor = self.header.section(SectionKind::Ownership);
        let fixed = descriptor
            .record_count
            .checked_mul(FILE_OWNERSHIP_RECORD_LEN)
            .ok_or(RepositoryStoreReadError::InvalidOwnership)?;
        for contribution in start..end {
            let offset = descriptor
                .offset
                .checked_add(fixed)
                .and_then(|value| {
                    contribution
                        .checked_mul(CONTRIBUTION_RECORD_LEN)
                        .and_then(|delta| value.checked_add(delta))
                })
                .ok_or(RepositoryStoreReadError::InvalidOwnership)?;
            let mut bytes = [0_u8; CONTRIBUTION_RECORD_LEN as usize];
            self.read_exact_at(offset, &mut bytes)?;

            let record_index = get_u64(&bytes, CONTRIBUTION_RECORD_INDEX_OFFSET);
            let kind = match bytes[CONTRIBUTION_KIND_OFFSET] {
                1 => ContributionKindView::Node,
                2 => ContributionKindView::Edge,
                3 => ContributionKindView::Unresolved,
                _ => return Err(RepositoryStoreReadError::InvalidOwnership),
            };
            let limit = match kind {
                ContributionKindView::Node => self.header.section(SectionKind::Nodes).record_count,
                ContributionKindView::Edge => self.header.section(SectionKind::Edges).record_count,
                ContributionKindView::Unresolved => {
                    self.header.section(SectionKind::Unresolved).record_count
                }
            };
            if record_index >= limit {
                return Err(RepositoryStoreReadError::InvalidOwnership);
            }
            visit(kind, record_index)?;
        }
        Ok(())
    }

    fn contribution_count(&self) -> Result<u64, RepositoryStoreReadError> {
        let descriptor = self.header.section(SectionKind::Ownership);
        let fixed = descriptor
            .record_count
            .checked_mul(FILE_OWNERSHIP_RECORD_LEN)
            .ok_or(RepositoryStoreReadError::InvalidOwnership)?;
        let tail = descriptor
            .byte_len
            .checked_sub(fixed)
            .ok_or(RepositoryStoreReadError::InvalidOwnership)?;
        if tail % CONTRIBUTION_RECORD_LEN != 0 {
            return Err(RepositoryStoreReadError::InvalidOwnership);
        }
        Ok(tail / CONTRIBUTION_RECORD_LEN)
    }

    fn ownership_record(
        &mut self,
        index: u64,
    ) -> Result<[u8; FILE_OWNERSHIP_RECORD_LEN as usize], RepositoryStoreReadError> {
        let descriptor = self.header.section(SectionKind::Ownership);
        if index >= descriptor.record_count {
            return Err(RepositoryStoreReadError::InvalidOwnership);
        }
        let offset = descriptor
            .offset
            .checked_add(
                index
                    .checked_mul(FILE_OWNERSHIP_RECORD_LEN)
                    .ok_or(RepositoryStoreReadError::InvalidOwnership)?,
            )
            .ok_or(RepositoryStoreReadError::InvalidOwnership)?;
        let mut bytes = [0_u8; FILE_OWNERSHIP_RECORD_LEN as usize];
        self.read_exact_at(offset, &mut bytes)?;
        Ok(bytes)
    }

    pub(super) fn node_record(
        &mut self,
        node_id: u32,
    ) -> Result<CompactNodeRecord, RepositoryStoreReadError> {
        let descriptor = self.header.section(SectionKind::Nodes);
        if u64::from(node_id) >= descriptor.record_count {
            return Err(RepositoryStoreReadError::InvalidNodeId(node_id));
        }
        let offset = descriptor
            .offset
            .checked_add(
                u64::from(node_id)
                    .checked_mul(NODE_RECORD_LEN)
                    .ok_or(RepositoryStoreReadError::InvalidNodeId(node_id))?,
            )
            .ok_or(RepositoryStoreReadError::InvalidNodeId(node_id))?;
        let mut bytes = [0_u8; NODE_RECORD_LEN as usize];
        self.read_exact_at(offset, &mut bytes)?;
        CompactNodeRecord::decode(&bytes).map_err(Into::into)
    }

    pub(super) fn edge_record(
        &mut self,
        index: u64,
    ) -> Result<CompactEdgeRecord, RepositoryStoreReadError> {
        let descriptor = self.header.section(SectionKind::Edges);
        if index >= descriptor.record_count {
            return Err(RepositoryStoreReadError::InvalidOwnership);
        }
        let offset = descriptor
            .offset
            .checked_add(
                index
                    .checked_mul(EDGE_RECORD_LEN)
                    .ok_or(RepositoryStoreReadError::InvalidOwnership)?,
            )
            .ok_or(RepositoryStoreReadError::InvalidOwnership)?;
        let mut bytes = [0_u8; EDGE_RECORD_LEN as usize];
        self.read_exact_at(offset, &mut bytes)?;
        CompactEdgeRecord::decode(&bytes).map_err(Into::into)
    }

    pub(super) fn unresolved_record(
        &mut self,
        index: u64,
    ) -> Result<CompactUnresolvedRecord, RepositoryStoreReadError> {
        let descriptor = self.header.section(SectionKind::Unresolved);
        if index >= descriptor.record_count {
            return Err(RepositoryStoreReadError::InvalidOwnership);
        }
        let offset = descriptor
            .offset
            .checked_add(
                index
                    .checked_mul(UNRESOLVED_RECORD_LEN)
                    .ok_or(RepositoryStoreReadError::InvalidOwnership)?,
            )
            .ok_or(RepositoryStoreReadError::InvalidOwnership)?;
        let mut bytes = [0_u8; UNRESOLVED_RECORD_LEN as usize];
        self.read_exact_at(offset, &mut bytes)?;
        CompactUnresolvedRecord::decode(&bytes).map_err(Into::into)
    }

    pub(super) fn node_key(&mut self, node_id: u32) -> Result<NodeKey, RepositoryStoreReadError> {
        let descriptor = self.header.section(SectionKind::Nodes);
        if u64::from(node_id) >= descriptor.record_count {
            return Err(RepositoryStoreReadError::InvalidNodeId(node_id));
        }
        let offset = descriptor
            .offset
            .checked_add(
                u64::from(node_id)
                    .checked_mul(NODE_RECORD_LEN)
                    .ok_or(RepositoryStoreReadError::InvalidNodeId(node_id))?,
            )
            .and_then(|value| value.checked_add(NODE_KEY_OFFSET as u64))
            .ok_or(RepositoryStoreReadError::InvalidNodeId(node_id))?;
        let mut bytes = [0_u8; 8];
        self.read_exact_at(offset, &mut bytes)?;
        Ok(NodeKey::from_u64(u64::from_le_bytes(bytes)))
    }

    pub(super) fn string(&mut self, id: StringId) -> Result<String, RepositoryStoreReadError> {
        let descriptor = self.header.section(SectionKind::Strings);
        if id == StringId::ABSENT {
            return Err(StoreFormatError::AbsentString.into());
        }
        if u64::from(id.0) >= descriptor.record_count {
            return Err(StoreFormatError::InvalidStringId(id.0).into());
        }
        let index_len = descriptor
            .record_count
            .checked_mul(STRING_INDEX_RECORD_LEN)
            .ok_or(StoreFormatError::SizeOverflow)?;
        let entry_offset = descriptor
            .offset
            .checked_add(u64::from(id.0) * STRING_INDEX_RECORD_LEN)
            .ok_or(StoreFormatError::SizeOverflow)?;
        let mut entry = [0_u8; STRING_INDEX_RECORD_LEN as usize];
        self.read_exact_at(entry_offset, &mut entry)?;
        if get_u32(&entry, 12) != 0 {
            return Err(StoreFormatError::MalformedStringTable.into());
        }
        let blob_offset = get_u64(&entry, STRING_ENTRY_OFFSET_OFFSET);
        let len = get_u32(&entry, STRING_ENTRY_LENGTH_OFFSET) as usize;
        let start = descriptor
            .offset
            .checked_add(index_len)
            .and_then(|value| value.checked_add(blob_offset))
            .ok_or(StoreFormatError::SizeOverflow)?;
        let section_end = descriptor
            .offset
            .checked_add(descriptor.byte_len)
            .ok_or(StoreFormatError::SizeOverflow)?;
        let end = start
            .checked_add(len as u64)
            .filter(|end| *end <= section_end)
            .ok_or(StoreFormatError::MalformedStringTable)?;
        let mut bytes = vec![0_u8; len];
        self.read_exact_at(start, &mut bytes)?;
        debug_assert_eq!(end, start + len as u64);
        String::from_utf8(bytes).map_err(|_| StoreFormatError::InvalidUtf8.into())
    }
}

use super::format::{SectionKind, node_kind_code};
use super::{RepositoryStoreFile, RepositoryStoreReadError};
use crate::repository::{
    CatalogueEntry, NodeKey, NodeKind, UnresolvedReferenceFact, normalize_repository_path,
};
use crate::synthetic::NodeId;

pub type SearchMatch = (usize, Vec<&'static str>, CatalogueEntry);

impl RepositoryStoreFile {
    pub fn node_occurrences(&mut self, id: NodeId) -> Result<u32, RepositoryStoreReadError> {
        Ok(self.node_record(id.0)?.occurrence_count)
    }
    pub fn node_kind_counts(
        &mut self,
    ) -> Result<std::collections::BTreeMap<String, u64>, RepositoryStoreReadError> {
        let mut result = std::collections::BTreeMap::new();
        for i in 0..self.header.section(SectionKind::KindIndex).record_count {
            let (code, _) = self.kind_index(i)?;
            let kind = super::format::node_kind_from_code(code)
                .ok_or(super::StoreFormatError::InvalidNodeKind(code))?;
            *result.entry(kind.as_str().to_owned()).or_default() += 1;
        }
        Ok(result)
    }
    pub fn unresolved_statistics(
        &mut self,
    ) -> Result<(std::collections::BTreeMap<String, u64>, u64), RepositoryStoreReadError> {
        let mut result = std::collections::BTreeMap::new();
        let mut calls = 0;
        let mut previous = None;
        for i in 0..self.unresolved_count() {
            let record = self.unresolved_record(i)?;
            if previous == Some(record) {
                continue;
            }
            previous = Some(record);
            let reason = if record.reason_code == super::format::UNKNOWN_REASON_CODE {
                self.string(record.unknown_reason)?
            } else {
                super::format::unresolved_reason_from_code(record.reason_code)
                    .ok_or(super::StoreFormatError::InvalidUnresolvedReason(
                        record.reason_code,
                    ))?
                    .as_str()
                    .to_owned()
            };
            *result.entry(reason).or_default() += 1;
            if record.relation_code
                == super::format::relation_code(&crate::repository::RelationKind::Calls)
            {
                calls += 1;
            }
        }
        Ok((result, calls))
    }
    pub fn search_matches(
        &mut self,
        query: &str,
        limit: usize,
    ) -> Result<(usize, Vec<SearchMatch>), RepositoryStoreReadError> {
        let mut ranks = super::search_scratch::SearchRanks::new();
        for id in 0..self.header.section(SectionKind::Strings).record_count {
            let value = self
                .string(super::StringId(id as u32))?
                .to_ascii_lowercase();
            let rank = if value == query {
                0
            } else if value.starts_with(query) {
                3
            } else if value.contains(query) {
                6
            } else {
                255
            };
            ranks.push(rank)?;
        }
        ranks.finish()?;
        let mut count = 0;
        let mut retained = std::collections::BinaryHeap::new();
        for id in 0..self.node_count() {
            let record = self.node_record(id)?;
            let fields = [
                ranks.get(record.name.0)?,
                ranks.get(record.qualified_name.0)?,
                ranks.get(record.path.0)?,
            ];
            let rank = fields
                .iter()
                .enumerate()
                .filter(|(_, value)| **value != 255)
                .map(|(index, value)| *value as usize + index)
                .min();
            if let Some(rank) = rank {
                count += 1;
                // Canonical string IDs preserve raw UTF-8 lexical order. Ranking
                // never seeks into string payloads for rejected candidates.
                let candidate = (rank, record.qualified_name.0, record.path.0, id, fields);
                if limit != 0 {
                    if retained.len() < limit {
                        retained.push(candidate);
                    } else if retained.peek().is_some_and(|worst| &candidate < worst) {
                        retained.pop();
                        retained.push(candidate);
                    }
                }
            }
        }
        let mut matches = Vec::new();
        for (rank, _, _, id, fields) in retained.into_sorted_vec() {
            let matched_fields = ["name", "qualified_name", "path"]
                .into_iter()
                .enumerate()
                .filter(|(index, _)| fields[*index] != 255)
                .map(|(_, field)| field)
                .collect();
            matches.push((
                rank,
                matched_fields,
                self.entry(NodeId(id))?.expect("scanned node"),
            ));
        }
        Ok((count, matches))
    }
    pub fn visit_unresolved(
        &mut self,
        source: Option<NodeKey>,
        reason: Option<&crate::repository::UnresolvedReason>,
        relation: Option<&crate::repository::RelationKind>,
        path: Option<&str>,
        mut visit: impl FnMut(u64, NodeId),
    ) -> Result<(), RepositoryStoreReadError> {
        let range = match source {
            Some(key) => self.unresolved_range(key)?,
            None => 0..self.unresolved_count(),
        };
        let path_ids = path.map(|path| self.node_ids_by_path(path)).transpose()?;
        let mut cursor = 0;
        let mut source_id = None;
        let mut last_key = None;
        let mut previous = None;
        for index in range {
            let record = self.unresolved_record(index)?;
            if previous == Some(record) {
                continue;
            }
            previous = Some(record);
            if last_key != Some(record.source) {
                source_id = if source.is_some() {
                    self.node_id(record.source)?
                } else {
                    while cursor < self.node_count() && self.node_key(cursor)? < record.source {
                        cursor += 1;
                    }
                    if cursor < self.node_count() && self.node_key(cursor)? == record.source {
                        Some(NodeId(cursor))
                    } else {
                        None
                    }
                };
                last_key = Some(record.source);
            }
            let id = source_id.ok_or(RepositoryStoreReadError::InvalidNodeId(cursor))?;
            if path_ids
                .as_ref()
                .is_some_and(|ids| ids.binary_search(&id).is_err())
            {
                continue;
            }
            if relation
                .is_some_and(|value| super::format::relation_code(value) != record.relation_code)
            {
                continue;
            }
            if let Some(reason) = reason {
                let wanted = super::format::unresolved_reason_code(reason);
                if wanted != record.reason_code {
                    continue;
                }
                if wanted == super::format::UNKNOWN_REASON_CODE
                    && self.string(record.unknown_reason)? != reason.as_str()
                {
                    continue;
                }
            }
            visit(index, id);
        }
        Ok(())
    }
    pub fn unique_unresolved_count(&mut self) -> Result<u64, RepositoryStoreReadError> {
        let mut previous = None;
        let mut count = 0;
        for i in 0..self.unresolved_count() {
            let record = self.unresolved_record(i)?;
            if previous != Some(record) {
                count += 1;
                previous = Some(record);
            }
        }
        Ok(count)
    }
    pub fn node_count(&self) -> u32 {
        self.header.section(SectionKind::Nodes).record_count as u32
    }
    pub fn unresolved_count(&self) -> u64 {
        self.header.section(SectionKind::Unresolved).record_count
    }
    pub fn entry(
        &mut self,
        id: NodeId,
    ) -> Result<Option<CatalogueEntry>, RepositoryStoreReadError> {
        if id.0 >= self.node_count() {
            return Ok(None);
        }
        let record = self.node_record(id.0)?;
        Ok(Some(CatalogueEntry {
            node_id: id,
            fact: self.materialize_node_record(record)?,
        }))
    }
    pub fn unresolved(
        &mut self,
        index: u64,
    ) -> Result<UnresolvedReferenceFact, RepositoryStoreReadError> {
        let record = self.unresolved_record(index)?;
        self.materialize_unresolved_record(record)
    }
    pub fn unresolved_range(
        &mut self,
        source: NodeKey,
    ) -> Result<std::ops::Range<u64>, RepositoryStoreReadError> {
        let count = self.unresolved_count();
        let start = self.bound(count, |store, i| {
            Ok(store.unresolved_record(i)?.source < source)
        })?;
        let end = self.bound(count, |store, i| {
            Ok(store.unresolved_record(i)?.source <= source)
        })?;
        Ok(start..end)
    }
    pub fn node_ids_by_name(
        &mut self,
        value: &str,
    ) -> Result<Vec<NodeId>, RepositoryStoreReadError> {
        self.string_index(SectionKind::NameIndex, value, false)
    }
    pub fn node_ids_by_path(
        &mut self,
        value: &str,
    ) -> Result<Vec<NodeId>, RepositoryStoreReadError> {
        let value = normalize_repository_path(value)?;
        self.string_index(SectionKind::PathIndex, &value, false)
    }
    pub fn node_ids_by_path_prefix(
        &mut self,
        value: &str,
    ) -> Result<Vec<NodeId>, RepositoryStoreReadError> {
        let value = normalize_repository_path(value)?;
        self.string_index(SectionKind::PathIndex, &value, true)
    }
    pub fn node_ids_by_kind(
        &mut self,
        kind: &NodeKind,
    ) -> Result<Vec<NodeId>, RepositoryStoreReadError> {
        let section = SectionKind::KindIndex;
        let target = node_kind_code(kind);
        let count = self.header.section(section).record_count;
        let start = self.bound(count, |store, i| Ok(store.kind_index(i)?.0 < target))?;
        let end = self.bound(count, |store, i| Ok(store.kind_index(i)?.0 <= target))?;
        let mut ids = Vec::new();
        for i in start..end {
            let (_, id) = self.kind_index(i)?;
            if self.node_record(id.0)?.kind_code != target {
                return Err(RepositoryStoreReadError::InvalidNodeId(id.0));
            }
            ids.push(id);
        }
        Ok(ids)
    }
    fn kind_index(&mut self, i: u64) -> Result<(u16, NodeId), RepositoryStoreReadError> {
        let mut bytes = [0; 8];
        self.read_exact_at(
            self.header.section(SectionKind::KindIndex).offset + i * 8,
            &mut bytes,
        )?;
        Ok((
            u16::from_le_bytes(bytes[..2].try_into().unwrap()),
            NodeId(u32::from_le_bytes(bytes[4..].try_into().unwrap())),
        ))
    }
    fn string_index_value(
        &mut self,
        section: SectionKind,
        i: u64,
    ) -> Result<(NodeId, String), RepositoryStoreReadError> {
        let mut bytes = [0; 4];
        self.read_exact_at(self.header.section(section).offset + i * 4, &mut bytes)?;
        let id = NodeId(u32::from_le_bytes(bytes));
        let record = self.node_record(id.0)?;
        let string = if section == SectionKind::NameIndex {
            record.name
        } else {
            record.path
        };
        Ok((id, self.string(string)?))
    }
    fn string_index(
        &mut self,
        section: SectionKind,
        value: &str,
        prefix: bool,
    ) -> Result<Vec<NodeId>, RepositoryStoreReadError> {
        let count = self.header.section(section).record_count;
        let start = self.bound(count, |store, i| {
            Ok(store.string_index_value(section, i)?.1.as_str() < value)
        })?;
        let mut ids = Vec::new();
        for i in start..count {
            let (id, candidate) = self.string_index_value(section, i)?;
            if candidate == value
                || (prefix
                    && candidate
                        .strip_prefix(value)
                        .is_some_and(|tail| tail.starts_with('/')))
            {
                ids.push(id);
            } else if !prefix || !candidate.starts_with(value) {
                break;
            }
        }
        if prefix {
            ids.sort_unstable();
        }
        Ok(ids)
    }
    fn bound(
        &mut self,
        mut high: u64,
        mut before: impl FnMut(&mut Self, u64) -> Result<bool, RepositoryStoreReadError>,
    ) -> Result<u64, RepositoryStoreReadError> {
        let mut low = 0;
        while low < high {
            let mid = low + (high - low) / 2;
            if before(self, mid)? {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        Ok(low)
    }
}

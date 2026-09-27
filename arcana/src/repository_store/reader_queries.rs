use crate::repository::{NodeKey, NodeKind, normalize_repository_path};

use super::format::{
    KIND_INDEX_KIND_OFFSET, KIND_INDEX_NODE_ID_OFFSET, KIND_INDEX_RECORD_LEN,
    NAME_INDEX_RECORD_LEN, PATH_INDEX_RECORD_LEN, SectionKind, node_kind_code,
};
use super::record_io::{get_u16, get_u32};
use super::{NodeRecordView, RepositoryStore, RepositoryStoreReadError};

impl RepositoryStore {
    pub fn lookup_by_key(
        &self,
        key: NodeKey,
    ) -> Result<Option<NodeRecordView<'_>>, RepositoryStoreReadError> {
        let count = self.node_count() as usize;
        let index = lower_bound_result(count, |index| {
            Ok::<bool, RepositoryStoreReadError>(self.node(index as u32)?.key() < key)
        })?;
        if index == count {
            return Ok(None);
        }
        let node = self.node(index as u32)?;
        Ok((node.key() == key).then_some(node))
    }

    pub fn lookup_by_name(
        &self,
        name: &str,
    ) -> Result<Vec<NodeRecordView<'_>>, RepositoryStoreReadError> {
        self.lookup_string_index(SectionKind::NameIndex, name, |node| node.name())
    }

    pub fn lookup_by_path(
        &self,
        path: &str,
    ) -> Result<Vec<NodeRecordView<'_>>, RepositoryStoreReadError> {
        let path = normalize_repository_path(path)?;
        self.lookup_string_index(SectionKind::PathIndex, &path, |node| node.path())
    }

    pub fn node_ids_by_path_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<u32>, RepositoryStoreReadError> {
        let prefix = normalize_repository_path(prefix)?;
        let bytes = self.section(SectionKind::PathIndex);
        let record_len = PATH_INDEX_RECORD_LEN as usize;
        let count = self.header.section(SectionKind::PathIndex).record_count as usize;
        let start = lower_bound_result(count, |index| {
            let id = get_u32(bytes, index * record_len);
            Ok::<bool, RepositoryStoreReadError>(self.node(id)?.path()? < prefix.as_str())
        })?;
        let mut ids = Vec::new();
        for index in start..count {
            let id = get_u32(bytes, index * record_len);
            let path = self.node(id)?.path()?;
            if path == prefix
                || path
                    .strip_prefix(&prefix)
                    .is_some_and(|tail| tail.starts_with('/'))
            {
                ids.push(id);
            } else if path > prefix.as_str() && !path.starts_with(&prefix) {
                break;
            }
        }
        ids.sort_unstable();
        Ok(ids)
    }

    pub fn node_ids_by_kind(&self, kind: &NodeKind) -> Result<Vec<u32>, RepositoryStoreReadError> {
        let target = node_kind_code(kind);
        let bytes = self.section(SectionKind::KindIndex);
        let count = self.header.section(SectionKind::KindIndex).record_count as usize;
        let start = lower_bound(count, |index| {
            get_u16(
                bytes,
                index * KIND_INDEX_RECORD_LEN as usize + KIND_INDEX_KIND_OFFSET,
            ) < target
        });
        let end = start
            + lower_bound(count - start, |offset| {
                get_u16(
                    bytes,
                    (start + offset) * KIND_INDEX_RECORD_LEN as usize + KIND_INDEX_KIND_OFFSET,
                ) == target
            });

        let mut ids = Vec::with_capacity(end - start);
        for index in start..end {
            let id = get_u32(
                bytes,
                index * KIND_INDEX_RECORD_LEN as usize + KIND_INDEX_NODE_ID_OFFSET,
            );
            let node = self.node(id)?;
            if node_kind_code(&node.kind()?) != target {
                return Err(RepositoryStoreReadError::InvalidNodeId(id));
            }
            ids.push(id);
        }
        Ok(ids)
    }

    fn lookup_string_index<F>(
        &self,
        kind: SectionKind,
        value: &str,
        field: F,
    ) -> Result<Vec<NodeRecordView<'_>>, RepositoryStoreReadError>
    where
        F: Fn(NodeRecordView<'_>) -> Result<&str, RepositoryStoreReadError> + Copy,
    {
        let bytes = self.section(kind);
        let record_len = index_record_len(kind);
        let count = self.header.section(kind).record_count as usize;
        let start = lower_bound_result(count, |index| {
            let id = get_u32(bytes, index * record_len);
            Ok::<bool, RepositoryStoreReadError>(field(self.node(id)?)? < value)
        })?;

        let mut output = Vec::new();
        for index in start..count {
            let id = get_u32(bytes, index * record_len);
            let node = self.node(id)?;
            if field(node)? != value {
                break;
            }
            output.push(node);
        }
        Ok(output)
    }
}

fn index_record_len(kind: SectionKind) -> usize {
    (match kind {
        SectionKind::NameIndex => NAME_INDEX_RECORD_LEN,
        SectionKind::PathIndex => PATH_INDEX_RECORD_LEN,
        _ => unreachable!(),
    }) as usize
}

fn lower_bound(mut count: usize, predicate: impl Fn(usize) -> bool) -> usize {
    let mut first = 0;
    while count > 0 {
        let step = count / 2;
        let mid = first + step;
        if predicate(mid) {
            first = mid + 1;
            count -= step + 1;
        } else {
            count = step;
        }
    }
    first
}

fn lower_bound_result<E>(
    mut count: usize,
    predicate: impl Fn(usize) -> Result<bool, E>,
) -> Result<usize, E> {
    let mut first = 0;
    while count > 0 {
        let step = count / 2;
        let mid = first + step;
        if predicate(mid)? {
            first = mid + 1;
            count -= step + 1;
        } else {
            count = step;
        }
    }
    Ok(first)
}

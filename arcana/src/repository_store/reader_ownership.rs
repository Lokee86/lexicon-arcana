use super::format::{
    CONTRIBUTION_KIND_OFFSET, CONTRIBUTION_RECORD_INDEX_OFFSET, CONTRIBUTION_RECORD_LEN,
    FILE_OWNERSHIP_RECORD_LEN, OWNERSHIP_CONTRIBUTION_COUNT_OFFSET,
    OWNERSHIP_CONTRIBUTION_START_OFFSET, OWNERSHIP_PATH_ID_OFFSET,
};
use super::record_io::{get_u32, get_u64};
use super::{RepositoryStoreReadError, StringId, StringTableView};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContributionKindView {
    Node,
    Edge,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnershipContributionView {
    pub kind: ContributionKindView,
    pub record_index: u64,
}

#[derive(Clone, Copy)]
pub struct OwnershipView<'a> {
    bytes: &'a [u8],
    file_count: usize,
    contribution_count: usize,
    strings: StringTableView<'a>,
    node_count: u64,
    edge_count: u64,
    unresolved_count: u64,
}

impl<'a> OwnershipView<'a> {
    pub(crate) fn new(
        bytes: &'a [u8],
        file_count: u64,
        strings: StringTableView<'a>,
        node_count: u64,
        edge_count: u64,
        unresolved_count: u64,
    ) -> Result<Self, RepositoryStoreReadError> {
        let file_count =
            usize::try_from(file_count).map_err(|_| RepositoryStoreReadError::InvalidOwnership)?;
        let fixed = file_count
            .checked_mul(FILE_OWNERSHIP_RECORD_LEN as usize)
            .ok_or(RepositoryStoreReadError::InvalidOwnership)?;
        if fixed > bytes.len() || (bytes.len() - fixed) % CONTRIBUTION_RECORD_LEN as usize != 0 {
            return Err(RepositoryStoreReadError::InvalidOwnership);
        }
        Ok(Self {
            bytes,
            file_count,
            contribution_count: (bytes.len() - fixed) / CONTRIBUTION_RECORD_LEN as usize,
            strings,
            node_count,
            edge_count,
            unresolved_count,
        })
    }

    pub fn contributions(
        self,
        path: &str,
    ) -> Result<Vec<OwnershipContributionView>, RepositoryStoreReadError> {
        let mut low = 0usize;
        let mut high = self.file_count;
        while low < high {
            let mid = (low + high) / 2;
            match self.path(mid)?.cmp(path) {
                std::cmp::Ordering::Less => low = mid + 1,
                std::cmp::Ordering::Greater => high = mid,
                std::cmp::Ordering::Equal => return self.read_contributions(mid),
            }
        }
        Ok(Vec::new())
    }

    fn path(self, index: usize) -> Result<&'a str, RepositoryStoreReadError> {
        let base = index
            .checked_mul(FILE_OWNERSHIP_RECORD_LEN as usize)
            .ok_or(RepositoryStoreReadError::InvalidOwnership)?;
        if base + FILE_OWNERSHIP_RECORD_LEN as usize > self.bytes.len() {
            return Err(RepositoryStoreReadError::InvalidOwnership);
        }
        self.strings.get(StringId(get_u32(
            self.bytes,
            base + OWNERSHIP_PATH_ID_OFFSET,
        )))
    }

    fn range(self, index: usize) -> Result<(u64, u64), RepositoryStoreReadError> {
        let base = index
            .checked_mul(FILE_OWNERSHIP_RECORD_LEN as usize)
            .ok_or(RepositoryStoreReadError::InvalidOwnership)?;
        let start = get_u64(self.bytes, base + OWNERSHIP_CONTRIBUTION_START_OFFSET);
        let count = get_u64(self.bytes, base + OWNERSHIP_CONTRIBUTION_COUNT_OFFSET);
        let end = start
            .checked_add(count)
            .filter(|end| *end <= self.contribution_count as u64)
            .ok_or(RepositoryStoreReadError::InvalidOwnership)?;
        Ok((start, end))
    }

    fn read_contributions(
        self,
        index: usize,
    ) -> Result<Vec<OwnershipContributionView>, RepositoryStoreReadError> {
        let (start, end) = self.range(index)?;
        let fixed = self.file_count * FILE_OWNERSHIP_RECORD_LEN as usize;
        let mut output = Vec::with_capacity((end - start) as usize);
        for contribution in start..end {
            let base = fixed + contribution as usize * CONTRIBUTION_RECORD_LEN as usize;
            let kind = match self.bytes[base + CONTRIBUTION_KIND_OFFSET] {
                1 => ContributionKindView::Node,
                2 => ContributionKindView::Edge,
                3 => ContributionKindView::Unresolved,
                _ => return Err(RepositoryStoreReadError::InvalidOwnership),
            };
            let record_index = get_u64(self.bytes, base + CONTRIBUTION_RECORD_INDEX_OFFSET);
            let limit = match kind {
                ContributionKindView::Node => self.node_count,
                ContributionKindView::Edge => self.edge_count,
                ContributionKindView::Unresolved => self.unresolved_count,
            };
            if record_index >= limit {
                return Err(RepositoryStoreReadError::InvalidOwnership);
            }
            output.push(OwnershipContributionView { kind, record_index });
        }
        Ok(output)
    }
}

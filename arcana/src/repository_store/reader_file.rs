use std::collections::{BTreeSet, VecDeque};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::repository::{NodeKey, RepositoryFacts, normalize_repository_path};

use super::format::RepositoryHeader;
use super::{ContributionKindView, RepositoryStoreReadError};

#[derive(Debug)]
pub struct RepositoryStoreFile {
    pub(super) file: File,
    pub(super) header: RepositoryHeader,
    path: PathBuf,
    artifact_checksum: u64,
    pages: VecDeque<(u64, Vec<u8>)>,
}

impl RepositoryStoreFile {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RepositoryStoreReadError> {
        let path = path.as_ref();
        let (file, header, artifact_checksum) =
            super::reader_file_validation::open_validated(path)?;
        Ok(Self {
            file,
            header,
            path: path.to_path_buf(),
            artifact_checksum,
            pages: VecDeque::new(),
        })
    }

    pub fn cached_bytes(&self) -> usize {
        self.pages.iter().map(|(_, page)| page.len()).sum()
    }

    pub const fn header(&self) -> &RepositoryHeader {
        &self.header
    }

    pub const fn artifact_checksum(&self) -> u64 {
        self.artifact_checksum
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub fn owned_node_keys(
        &mut self,
        paths: &[String],
    ) -> Result<Vec<NodeKey>, RepositoryStoreReadError> {
        let mut node_ids = BTreeSet::new();
        for path in paths {
            let path = normalize_repository_path(path)?;
            if let Some(index) = self.find_ownership(&path)? {
                self.for_each_owned_contribution(index, |kind, record_index| {
                    if kind == ContributionKindView::Node {
                        node_ids.insert(
                            u32::try_from(record_index)
                                .map_err(|_| RepositoryStoreReadError::InvalidOwnership)?,
                        );
                    }
                    Ok(())
                })?;
            }
        }
        node_ids
            .into_iter()
            .map(|node_id| self.node_key(node_id))
            .collect()
    }

    pub fn owned_facts(
        &mut self,
        paths: &[String],
    ) -> Result<RepositoryFacts, RepositoryStoreReadError> {
        let mut nodes = BTreeSet::new();
        let mut edges = BTreeSet::new();
        let mut unresolved = BTreeSet::new();

        for path in paths {
            let path = normalize_repository_path(path)?;
            if let Some(index) = self.find_ownership(&path)? {
                self.for_each_owned_contribution(index, |kind, record_index| {
                    match kind {
                        ContributionKindView::Node => {
                            nodes.insert(
                                u32::try_from(record_index)
                                    .map_err(|_| RepositoryStoreReadError::InvalidOwnership)?,
                            );
                        }
                        ContributionKindView::Edge => {
                            edges.insert(record_index);
                        }
                        ContributionKindView::Unresolved => {
                            unresolved.insert(record_index);
                        }
                    }
                    Ok(())
                })?;
            }
        }

        let mut facts = RepositoryFacts::default();
        for node_id in nodes {
            let record = self.node_record(node_id)?;
            let fact = self.materialize_node_record(record)?;
            facts.nodes.reserve(record.occurrence_count as usize);
            for _ in 0..record.occurrence_count {
                facts.nodes.push(fact.clone());
            }
        }
        for index in edges {
            let record = self.edge_record(index)?;
            facts.edges.push(self.materialize_edge_record(record)?);
        }
        for index in unresolved {
            let record = self.unresolved_record(index)?;
            facts
                .unresolved
                .push(self.materialize_unresolved_record(record)?);
        }
        Ok(facts)
    }

    pub(super) fn read_exact_at(
        &mut self,
        offset: u64,
        bytes: &mut [u8],
    ) -> Result<(), RepositoryStoreReadError> {
        // A bounded 4 MiB page cache serves interleaved record/string streams.
        const PAGE: u64 = 64 * 1024;
        let mut position = offset;
        let mut written = 0;
        while written < bytes.len() {
            let base = position / PAGE * PAGE;
            let index = match self.pages.iter().position(|(key, _)| *key == base) {
                Some(index) => index,
                None => {
                    let len = self
                        .header
                        .file_len
                        .checked_sub(base)
                        .ok_or(RepositoryStoreReadError::InvalidOwnership)?
                        .min(PAGE) as usize;
                    let mut page = vec![0; len];
                    self.file.seek(SeekFrom::Start(base))?;
                    self.file.read_exact(&mut page)?;
                    if self.pages.len() == 64 {
                        self.pages.pop_front();
                    }
                    self.pages.push_back((base, page));
                    self.pages.len() - 1
                }
            };
            let page = self.pages.remove(index).expect("located page");
            let start = (position - base) as usize;
            let count = (page.1.len() - start).min(bytes.len() - written);
            if count == 0 {
                return Err(RepositoryStoreReadError::InvalidOwnership);
            }
            bytes[written..written + count].copy_from_slice(&page.1[start..start + count]);
            self.pages.push_back(page);
            position += count as u64;
            written += count;
        }
        Ok(())
    }
}

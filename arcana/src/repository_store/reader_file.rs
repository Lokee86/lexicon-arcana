use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::repository::{NodeKey, RepositoryFacts, normalize_repository_path};

use super::format::RepositoryHeader;
use super::{ContributionKindView, RepositoryStoreReadError};

pub struct RepositoryStoreFile {
    pub(super) file: File,
    pub(super) header: RepositoryHeader,
    artifact_checksum: u64,
}

impl RepositoryStoreFile {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RepositoryStoreReadError> {
        let (file, header, artifact_checksum) =
            super::reader_file_validation::open_validated(path.as_ref())?;
        Ok(Self {
            file,
            header,
            artifact_checksum,
        })
    }

    pub const fn header(&self) -> &RepositoryHeader {
        &self.header
    }

    pub const fn artifact_checksum(&self) -> u64 {
        self.artifact_checksum
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
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(bytes)?;
        Ok(())
    }
}

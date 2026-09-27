use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::repository::{NodeKey, normalize_repository_path};

use super::RepositoryStoreReadError;
use super::format::RepositoryHeader;

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
                self.collect_owned_nodes(index, &mut node_ids)?;
            }
        }
        node_ids
            .into_iter()
            .map(|node_id| self.node_key(node_id))
            .collect()
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

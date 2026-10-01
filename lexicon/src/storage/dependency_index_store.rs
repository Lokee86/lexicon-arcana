use std::fs;
use std::path::PathBuf;

use serde::{Serialize, de::DeserializeOwned};

use super::dependency_index_model::{
    DOMAIN, IndexRoot, LegacyBootstrap, VERSION, language_signature,
};
use super::digest::domain_id;
use super::io::{write_atomic, write_immutable};
use super::store::validate_storage_id;
use super::{LanguageEntry, StorageError, Store};

impl Store {
    pub(super) fn write_index_object<T: Serialize>(
        &self,
        value: &T,
    ) -> Result<String, StorageError> {
        let bytes = serde_json::to_vec(value)?;
        let id = domain_id(DOMAIN, &bytes);
        write_immutable(&self.topology_path(&id), &bytes)?;
        Ok(id)
    }

    pub(super) fn load_index_object<T: DeserializeOwned>(
        &self,
        id: &str,
    ) -> Result<T, StorageError> {
        validate_storage_id(id)?;
        let bytes = fs::read(self.topology_path(id))?;
        if domain_id(DOMAIN, &bytes) != id {
            return Err(StorageError::Verification(id.to_owned()));
        }
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub(super) fn validated_index(
        &self,
        id: &str,
        entry: &LanguageEntry,
    ) -> Result<IndexRoot, StorageError> {
        let root: IndexRoot = self.load_index_object(id)?;
        if root.version != VERSION || root.language_signature != language_signature(entry)? {
            return Err(StorageError::Verification(id.to_owned()));
        }
        for partition in root
            .files
            .values()
            .chain(root.nodes.values())
            .chain(root.references.values())
            .chain(root.unresolved.values())
        {
            validate_storage_id(partition)?;
        }
        Ok(root)
    }

    pub(super) fn topology_path(&self, id: &str) -> PathBuf {
        let hex = id.strip_prefix("sha256:").unwrap_or(id);
        self.root()
            .join("topology")
            .join("objects")
            .join(&hex[..2])
            .join(&hex[2..])
    }

    pub(super) fn bootstrap_path(&self, snapshot_id: &str, language: &str) -> PathBuf {
        let language_id = domain_id(
            b"lexicon:dependency-bootstrap-language:v1\0",
            language.as_bytes(),
        );
        self.root()
            .join("topology")
            .join("bootstrap")
            .join(snapshot_id.strip_prefix("sha256:").unwrap_or(snapshot_id))
            .join(format!(
                "{}.json",
                language_id.trim_start_matches("sha256:")
            ))
    }

    pub(super) fn read_bootstrap(
        &self,
        snapshot_id: &str,
        entry: &LanguageEntry,
    ) -> Result<Option<String>, StorageError> {
        let path = self.bootstrap_path(snapshot_id, &entry.language);
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let bootstrap: LegacyBootstrap = serde_json::from_slice(&bytes)?;
        if bootstrap.version != VERSION
            || bootstrap.snapshot_id != snapshot_id
            || bootstrap.language_signature != language_signature(entry)?
        {
            return Err(StorageError::Verification(snapshot_id.to_owned()));
        }
        self.validated_index(&bootstrap.index_id, entry)?;
        Ok(Some(bootstrap.index_id))
    }

    pub(super) fn publish_bootstrap(
        &self,
        snapshot_id: &str,
        entry: &LanguageEntry,
        index_id: &str,
    ) -> Result<(), StorageError> {
        self.validated_index(index_id, entry)?;
        let bootstrap = LegacyBootstrap {
            version: VERSION,
            snapshot_id: snapshot_id.to_owned(),
            language_signature: language_signature(entry)?,
            index_id: index_id.to_owned(),
        };
        write_atomic(
            &self.bootstrap_path(snapshot_id, &entry.language),
            &serde_json::to_vec(&bootstrap)?,
        )
    }
}

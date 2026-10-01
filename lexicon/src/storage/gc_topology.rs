use std::collections::BTreeSet;
use std::fs;
use std::io;

use super::store::validate_storage_id;
use super::{SnapshotManifest, StorageError, Store};

impl Store {
    pub(super) fn refuse_gc_during_pending(&self) -> Result<(), StorageError> {
        match self.pending() {
            Err(StorageError::NoPendingPublication) => Ok(()),
            Ok(_) => Err(StorageError::Operation(
                "cannot collect Lexicon objects while PENDING exists".into(),
            )),
            Err(error) => Err(error),
        }
    }

    pub(super) fn add_topology_references(
        &self,
        snapshot: &str,
        manifest: &SnapshotManifest,
        retained: &mut BTreeSet<String>,
    ) -> Result<(), StorageError> {
        for entry in manifest.languages.as_deref().unwrap_or_default() {
            let id = if !entry.dependency_index_id.is_empty() {
                Some(entry.dependency_index_id.clone())
            } else {
                self.read_bootstrap(snapshot, entry)?
            };
            let Some(id) = id else { continue };
            let root = self.validated_index(&id, entry)?;
            retained.insert(id);
            for id in root
                .files
                .values()
                .chain(root.nodes.values())
                .chain(root.references.values())
                .chain(root.unresolved.values())
                .chain(root.shared_paths.values())
            {
                let _: serde_json::Value = self.load_index_object(id)?;
                retained.insert(id.clone());
            }
        }
        Ok(())
    }

    pub(super) fn list_topology_objects(&self) -> Result<BTreeSet<String>, StorageError> {
        let mut result = BTreeSet::new();
        let base = self.root().join("topology/objects");
        let directories = match fs::read_dir(base) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(result),
            Err(error) => return Err(error.into()),
        };
        for directory in directories {
            let directory = directory?;
            let prefix = directory.file_name().to_string_lossy().into_owned();
            if !directory.file_type()?.is_dir()
                || prefix.len() != 2
                || !prefix.bytes().all(|b| b.is_ascii_hexdigit())
            {
                continue;
            }
            for file in fs::read_dir(directory.path())? {
                let file = file?;
                if !file.file_type()?.is_file() {
                    continue;
                }
                let id = format!("sha256:{prefix}{}", file.file_name().to_string_lossy());
                if validate_storage_id(&id).is_ok() {
                    result.insert(id);
                }
            }
        }
        Ok(result)
    }

    pub(super) fn list_bootstrap_snapshots(&self) -> Result<BTreeSet<String>, StorageError> {
        let mut result = BTreeSet::new();
        let base = self.root().join("topology/bootstrap");
        let entries = match fs::read_dir(base) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(result),
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let id = format!("sha256:{}", entry.file_name().to_string_lossy());
            validate_storage_id(&id)?;
            result.insert(id);
        }
        Ok(result)
    }
}

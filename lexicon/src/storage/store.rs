use std::fs;
use std::path::{Path, PathBuf};

use super::binary::{ObjectView, encode_view, is_binary_object};
use super::io::{write_atomic, write_immutable};
use super::snapshot::snapshot_id_bytes;
use super::{
    FactObject, SnapshotManifest, StorageError, StoreLock, decode_object, encode_object, object_id,
    snapshot_bytes,
};

#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn lock(&self) -> Result<StoreLock, StorageError> {
        StoreLock::acquire(&self.root)
    }

    pub fn write_object(&self, object: &FactObject) -> Result<String, StorageError> {
        let encoded = encode_object(object)?;
        self.write_encoded_object(encoded)
    }

    pub(crate) fn write_object_view(
        &self,
        object: &ObjectView<'_>,
    ) -> Result<String, StorageError> {
        let encoded = encode_view(object)?;
        self.write_encoded_object(encoded)
    }

    fn write_encoded_object(&self, encoded: Vec<u8>) -> Result<String, StorageError> {
        let id = object_id(&encoded);
        write_immutable(&self.object_path(&id), &encoded)?;
        Ok(id)
    }

    pub fn load_object(&self, id: &str) -> Result<FactObject, StorageError> {
        let bytes = self.load_verified_object_bytes(id)?;
        decode_object(&bytes)
    }

    pub fn load_node_facts(
        &self,
        id: &str,
    ) -> Result<(FactObject, Vec<crate::NodeRecord>), StorageError> {
        let bytes = self.load_verified_object_bytes(id)?;
        super::decode_node_facts(&bytes)
    }

    pub fn publish(&self, manifest: &SnapshotManifest) -> Result<String, StorageError> {
        // Publish a manifest only after its referenced immutable topology is
        // fully written and verified. Pending recovery passes through here too.
        for entry in manifest.languages.as_deref().unwrap_or_default() {
            if entry.dependency_index_id.is_empty() {
                continue;
            }
            let root = self.validated_index(&entry.dependency_index_id, entry)?;
            for id in root
                .files
                .values()
                .chain(root.nodes.values())
                .chain(root.references.values())
                .chain(root.unresolved.values())
                .chain(root.shared_paths.values())
            {
                let _: serde_json::Value = self.load_index_object(id)?;
            }
        }
        let canonical = snapshot_bytes(manifest)?;
        let id = snapshot_id_bytes(&canonical);
        let mut stored = canonical;
        stored.push(b'\n');
        write_immutable(&self.snapshot_path(&id), &stored)?;
        write_atomic(&self.root.join("CURRENT"), format!("{id}\n").as_bytes())?;
        Ok(id)
    }

    pub fn load_snapshot(&self, id: &str) -> Result<SnapshotManifest, StorageError> {
        validate_storage_id(id)?;
        let bytes = fs::read(self.snapshot_path(id))?;
        let canonical = trim_ascii(&bytes);
        if snapshot_id_bytes(canonical) != id {
            return Err(StorageError::Verification(id.to_owned()));
        }
        let manifest: SnapshotManifest = serde_json::from_slice(canonical)?;
        if manifest.version != super::SNAPSHOT_VERSION {
            return Err(StorageError::UnsupportedSnapshotVersion(manifest.version));
        }
        Ok(manifest)
    }

    pub fn current(&self) -> Result<(String, SnapshotManifest), StorageError> {
        let bytes = match fs::read(self.root.join("CURRENT")) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(StorageError::NoCurrentSnapshot);
            }
            Err(error) => return Err(error.into()),
        };
        let raw = trim_ascii(&bytes);
        if raw.is_empty() {
            return Err(StorageError::NoCurrentSnapshot);
        }
        let id = std::str::from_utf8(raw)
            .map_err(|_| StorageError::InvalidId(String::from("<non-utf8>")))?
            .to_owned();
        let manifest = self.load_snapshot(&id)?;
        Ok((id, manifest))
    }

    pub fn object_path(&self, id: &str) -> PathBuf {
        let hex = id.strip_prefix("sha256:").unwrap_or(id);
        if hex.len() < 3 {
            return self.root.join("objects").join(hex);
        }
        self.root.join("objects").join(&hex[..2]).join(&hex[2..])
    }

    pub fn snapshot_path(&self, id: &str) -> PathBuf {
        let hex = id.strip_prefix("sha256:").unwrap_or(id);
        self.root.join("snapshots").join(format!("{hex}.json"))
    }

    pub(crate) fn pending_path(&self) -> PathBuf {
        self.root.join("PENDING")
    }

    fn load_verified_object_bytes(&self, id: &str) -> Result<Vec<u8>, StorageError> {
        validate_storage_id(id)?;
        let bytes = fs::read(self.object_path(id))?;
        let canonical = if is_binary_object(&bytes) {
            bytes
        } else {
            trim_ascii(&bytes).to_vec()
        };
        if object_id(&canonical) != id {
            return Err(StorageError::Verification(id.to_owned()));
        }
        Ok(canonical)
    }
}

pub(crate) fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map_or(start, |index| index + 1);
    &bytes[start..end]
}

pub(crate) fn validate_storage_id(id: &str) -> Result<(), StorageError> {
    crate::validate_sha256_id(id).map_err(|_| StorageError::InvalidId(id.to_owned()))
}

use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{ManifestError, SnapshotManifest};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

impl SnapshotManifest {
    /// Writes a new manifest without replacing an existing path.
    pub fn write_to(&self, path: impl AsRef<Path>) -> Result<(), ManifestError> {
        let path = path.as_ref();
        let encoded = self.encode()?;
        if path.try_exists()? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("manifest already exists: {}", path.display()),
            )
            .into());
        }

        let temporary = temporary_path(path);
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(encoded.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    /// Reads and strictly parses a manifest from disk.
    pub fn read_from(path: impl AsRef<Path>) -> Result<Self, ManifestError> {
        Self::decode(&fs::read_to_string(path)?)
    }
}

/// Writes a new manifest without replacing an existing path.
pub fn write_manifest(
    path: impl AsRef<Path>,
    manifest: &SnapshotManifest,
) -> Result<(), ManifestError> {
    manifest.write_to(path)
}

/// Reads and strictly parses a manifest from disk.
pub fn read_manifest(path: impl AsRef<Path>) -> Result<SnapshotManifest, ManifestError> {
    SnapshotManifest::read_from(path)
}

fn temporary_path(path: &Path) -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut name = path
        .file_name()
        .map_or_else(|| OsString::from("manifest"), OsString::from);
    name.push(format!(".tmp.{}.{}", std::process::id(), sequence));
    path.with_file_name(name)
}

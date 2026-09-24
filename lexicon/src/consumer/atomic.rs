use std::io::Write;
use std::path::Path;

use atomicwrites::{AllowOverwrite, AtomicFile};

use crate::LexiconError;

pub(super) fn write_atomic(path: &Path, data: &[u8]) -> Result<(), LexiconError> {
    let parent = path
        .parent()
        .ok_or_else(|| LexiconError::new("consumer path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    AtomicFile::new(path, AllowOverwrite)
        .write(|file| file.write_all(data))
        .map_err(|error| {
            LexiconError::new(format!("replace consumer file {}: {error}", path.display()))
        })
}

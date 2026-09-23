use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

use super::StorageError;

pub struct StoreLock {
    file: File,
}

impl StoreLock {
    pub(crate) fn acquire(root: &Path) -> Result<Self, StorageError> {
        fs::create_dir_all(root)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("LOCK"))?;
        match FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Self { file }),
            Err(error) if is_busy(&error) => Err(StorageError::Busy),
            Err(error) => Err(error.into()),
        }
    }
}

fn is_busy(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::WouldBlock || matches!(error.raw_os_error(), Some(32 | 33))
}

impl Drop for StoreLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

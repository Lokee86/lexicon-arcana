use atomicwrites::replace_atomic;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::StorageError;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn write_immutable(path: &Path, data: &[u8]) -> Result<(), StorageError> {
    match fs::read(path) {
        Ok(existing) if existing == data => return Ok(()),
        Ok(_) => return Err(StorageError::Collision(path.display().to_string())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let temporary = write_temporary(path, data)?;
    match fs::rename(&temporary, path) {
        Ok(()) => sync_parent(path),
        Err(error) => {
            let matched = fs::read(path).is_ok_and(|existing| existing == data);
            let _ = fs::remove_file(&temporary);
            if matched { Ok(()) } else { Err(error.into()) }
        }
    }
}

pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> Result<(), StorageError> {
    let temporary = write_temporary(path, data)?;
    if let Err(error) = replace_atomic(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    sync_parent(path)
}

fn write_temporary(destination: &Path, data: &[u8]) -> Result<PathBuf, StorageError> {
    let directory = destination.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(directory)?;

    for _ in 0..128 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!(".lexicon-tmp-{}-{sequence}", std::process::id()));
        let file = OpenOptions::new().write(true).create_new(true).open(&path);
        match file {
            Ok(mut file) => {
                if let Err(error) = set_file_mode(&file) {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(error.into());
                }
                if let Err(error) = write_and_sync(&mut file, data) {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(error.into());
                }
                drop(file);
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate Lexicon temporary file",
    )
    .into())
}

fn write_and_sync(file: &mut File, data: &[u8]) -> io::Result<()> {
    file.write_all(data)?;
    file.sync_all()
}

#[cfg(unix)]
fn set_file_mode(file: &File) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(0o644))
}

#[cfg(not(unix))]
fn set_file_mode(_file: &File) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<(), StorageError> {
    let directory = File::open(path.parent().unwrap_or_else(|| Path::new(".")))?;
    directory.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent(_path: &Path) -> Result<(), StorageError> {
    Ok(())
}

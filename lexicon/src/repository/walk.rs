use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::languages::for_path;

use super::{IgnorePolicy, RepositoryError};

pub(crate) fn relevant_files(
    source_root: &Path,
    start: &Path,
    policy: &IgnorePolicy,
) -> Result<BTreeMap<PathBuf, PathBuf>, RepositoryError> {
    let mut desired = BTreeMap::new();
    walk(source_root, start, policy, &mut desired)?;
    Ok(desired)
}

fn walk(
    source_root: &Path,
    directory: &Path,
    policy: &IgnorePolicy,
    desired: &mut BTreeMap<PathBuf, PathBuf>,
) -> Result<(), RepositoryError> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let mut entries = entries.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        if policy.ignored(&path, file_type.is_dir()) {
            continue;
        }
        if file_type.is_dir() {
            walk(source_root, &path, policy, desired)?;
            continue;
        }
        if file_type.is_symlink() || for_path(path.to_string_lossy().as_ref()).is_empty() {
            continue;
        }
        let relative = path
            .strip_prefix(source_root)
            .map_err(|_| RepositoryError::new("source file escaped repository root"))?;
        desired.insert(relative.to_path_buf(), path);
    }
    Ok(())
}

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::languages::for_path;

use super::walk::relevant_files;
use super::{IgnorePolicy, RepositoryError};

#[derive(Debug, Clone)]
pub struct SourceMirror {
    root: PathBuf,
}

impl SourceMirror {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn sync_all(&self, source: &Path) -> Result<(), RepositoryError> {
        let source = absolute(source)?;
        let policy = IgnorePolicy::load(&source)?;
        let desired = relevant_files(&source, &source, &policy)?;
        self.copy_all(&desired)?;
        self.remove_missing(&desired)
    }

    pub fn sync_paths(&self, source: &Path, paths: &[PathBuf]) -> Result<(), RepositoryError> {
        let source = absolute(source)?;
        let policy = IgnorePolicy::load(&source)?;
        let mut paths = paths.to_vec();
        paths.sort();
        for path in paths {
            let absolute = if path.is_absolute() {
                clean(&path)
            } else {
                clean(&source.join(path))
            };
            let Ok(relative) = absolute.strip_prefix(&source) else {
                continue;
            };
            if relative.as_os_str().is_empty() || policy.ignored(&absolute, false) {
                continue;
            }
            let metadata = match fs::symlink_metadata(&absolute) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let _ = fs::remove_dir_all(self.root.join(relative));
                    let _ = fs::remove_file(self.root.join(relative));
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            if policy.ignored(&absolute, metadata.is_dir()) {
                continue;
            }
            if metadata.is_dir() {
                self.sync_directory(&source, relative, &policy)?;
            } else if metadata.file_type().is_symlink()
                || for_path(relative.to_string_lossy().as_ref()).is_empty()
            {
                let _ = fs::remove_file(self.root.join(relative));
            } else {
                self.copy(relative, &absolute)?;
            }
        }
        Ok(())
    }

    fn sync_directory(
        &self,
        source: &Path,
        relative: &Path,
        policy: &IgnorePolicy,
    ) -> Result<(), RepositoryError> {
        let desired = relevant_files(source, &source.join(relative), policy)?;
        for (path, source_path) in &desired {
            self.copy(path, source_path)?;
        }
        self.remove_missing_under(relative, &desired)
    }

    fn copy_all(&self, desired: &BTreeMap<PathBuf, PathBuf>) -> Result<(), RepositoryError> {
        for (relative, source) in desired {
            self.copy(relative, source)?;
        }
        Ok(())
    }

    fn copy(&self, relative: &Path, source: &Path) -> Result<(), RepositoryError> {
        let data = fs::read(source)?;
        let destination = self.root.join(relative);
        if fs::read(&destination).is_ok_and(|existing| existing == data) {
            return Ok(());
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = PathBuf::from(format!("{}.lexicon-tmp", destination.display()));
        fs::write(&temporary, data)?;
        if let Err(first) = fs::rename(&temporary, &destination) {
            let _ = fs::remove_file(&destination);
            fs::rename(&temporary, &destination).map_err(|retry| {
                RepositoryError::new(format!(
                    "replace mirror file {}: {retry}; initial error: {first}",
                    destination.display()
                ))
            })?;
        }
        Ok(())
    }

    fn remove_missing(&self, desired: &BTreeMap<PathBuf, PathBuf>) -> Result<(), RepositoryError> {
        self.remove_missing_from(&self.root, desired)
    }

    fn remove_missing_under(
        &self,
        relative: &Path,
        desired: &BTreeMap<PathBuf, PathBuf>,
    ) -> Result<(), RepositoryError> {
        self.remove_missing_from(&self.root.join(relative), desired)
    }

    fn remove_missing_from(
        &self,
        start: &Path,
        desired: &BTreeMap<PathBuf, PathBuf>,
    ) -> Result<(), RepositoryError> {
        let mut directories = Vec::new();
        collect_mirror_entries(&self.root, start, desired, &mut directories)?;
        directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        for directory in directories {
            if fs::read_dir(&directory).is_ok_and(|mut entries| entries.next().is_none()) {
                let _ = fs::remove_dir(&directory);
            }
        }
        Ok(())
    }
}

fn collect_mirror_entries(
    root: &Path,
    current: &Path,
    desired: &BTreeMap<PathBuf, PathBuf>,
    directories: &mut Vec<PathBuf>,
) -> Result<(), RepositoryError> {
    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            directories.push(path.clone());
            collect_mirror_entries(root, &path, desired, directories)?;
        } else if let Ok(relative) = path.strip_prefix(root)
            && !desired.contains_key(relative)
        {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn absolute(path: &Path) -> Result<PathBuf, RepositoryError> {
    if path.is_absolute() {
        return Ok(clean(path));
    }
    std::env::current_dir()
        .map(|current| clean(&current.join(path)))
        .map_err(RepositoryError::from)
}

fn clean(path: &Path) -> PathBuf {
    crate::config::clean_path(path)
}

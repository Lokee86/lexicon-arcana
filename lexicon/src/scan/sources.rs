use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::SourceFile;
use crate::languages::for_path;

pub(crate) fn language_present(source_root: &Path, language: &str) -> Result<bool, std::io::Error> {
    Ok(!language_sources(source_root, language)?.is_empty())
}

pub(crate) fn language_sources(
    source_root: &Path,
    language: &str,
) -> Result<Vec<SourceFile>, std::io::Error> {
    let mut paths = Vec::new();
    collect_paths(source_root, source_root, language, &mut paths)?;
    paths.sort();
    paths
        .into_iter()
        .map(|path| read_source(source_root, &path))
        .collect()
}

pub(crate) fn selected_sources(
    source_root: &Path,
    paths: &[String],
) -> Result<Vec<SourceFile>, std::io::Error> {
    let paths: BTreeSet<String> = paths
        .iter()
        .filter(|path| !path.is_empty())
        .map(|path| path.replace('\\', "/"))
        .collect();
    paths
        .iter()
        .map(|path| read_source(source_root, path))
        .collect()
}

fn collect_paths(
    source_root: &Path,
    current: &Path,
    language: &str,
    paths: &mut Vec<String>,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_paths(source_root, &path, language, paths)?;
            continue;
        }
        let relative = path
            .strip_prefix(source_root)
            .expect("walked path must remain beneath source root")
            .to_string_lossy()
            .replace('\\', "/");
        if for_path(&relative)
            .iter()
            .any(|candidate| candidate == language)
        {
            paths.push(relative);
        }
    }
    Ok(())
}

fn read_source(root: &Path, path: &str) -> Result<SourceFile, std::io::Error> {
    Ok(SourceFile {
        path: path.to_owned(),
        content: fs::read(root.join(path_from_slash(path)))?,
    })
}

fn path_from_slash(value: &str) -> PathBuf {
    value.split('/').collect()
}

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::{AdapterError, content_id};

use super::dxl::lotus_script_content;
use super::model::{Repository, SourceFile};

const EXCLUDED: &[&str] = &[
    ".git",
    ".worktrees",
    ".workingtrees",
    ".ddocs",
    ".lexicon",
    ".arcana",
    ".grimoire",
    ".pitlord",
    ".cantrip",
    ".homunculus",
    ".incubus",
    ".ritual",
    ".warlock",
    "node_modules",
    "target",
    "__pycache__",
    ".pytest_cache",
    ".bundle",
    "vendor",
    "build",
    "dist",
    "bin",
    "obj",
];

pub fn discover(root: &Path) -> Result<Repository, AdapterError> {
    let root = fs::canonicalize(root)
        .map_err(|error| AdapterError::new(format!("resolve repository: {error}")))?;
    if !root.is_dir() {
        return Err(AdapterError::new("repository path is not a directory"));
    }
    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("repository")
        .to_owned();

    let mut sources = Vec::new();
    walk(&root, &root, &mut sources)?;
    sources.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(Repository {
        name,
        directories: source_directories(&sources),
        sources,
    })
}

fn walk(root: &Path, current: &Path, sources: &mut Vec<SourceFile>) -> Result<(), AdapterError> {
    let mut entries = fs::read_dir(current)
        .map_err(AdapterError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AdapterError::from)?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let kind = entry.file_type().map_err(AdapterError::from)?;
        if kind.is_dir() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if EXCLUDED.contains(&name.as_str()) {
                continue;
            }
            walk(root, &path, sources)?;
            continue;
        }
        if !kind.is_file() {
            continue;
        }

        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{}", value.to_ascii_lowercase()))
            .unwrap_or_default();
        if !matches!(extension.as_str(), ".ls" | ".lss" | ".lsa" | ".lsdb") {
            continue;
        }

        let raw = fs::read(&path).map_err(AdapterError::from)?;
        let Some(content) = lotus_script_content(&extension, &raw) else {
            continue;
        };
        sources.push(SourceFile {
            path: relative(root, &path),
            raw_content_id: content_id(&raw),
            content,
        });
    }
    Ok(())
}

fn source_directories(sources: &[SourceFile]) -> Vec<String> {
    let mut values = BTreeSet::from([".".to_owned()]);
    for source in sources {
        let mut directory = Path::new(&source.path)
            .parent()
            .map(|value| value.to_string_lossy().replace('\\', "/"))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| ".".into());
        while directory != "." && !directory.is_empty() {
            values.insert(directory.clone());
            directory = Path::new(&directory)
                .parent()
                .map(|value| value.to_string_lossy().replace('\\', "/"))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| ".".into());
        }
    }
    values.into_iter().collect()
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

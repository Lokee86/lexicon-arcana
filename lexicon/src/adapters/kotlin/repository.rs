use std::fs;
use std::path::Path;

use crate::{AdapterError, SourceSpan};

use super::model::{ManifestFile, SourceFile};

const EXCLUDED: &[&str] = &[
    ".arcana",
    ".bundle",
    ".cantrip",
    ".ddocs",
    ".git",
    ".godot",
    ".gradle",
    ".grimoire",
    ".homunculus",
    ".idea",
    ".import",
    ".incubus",
    ".kotlin",
    ".lexicon",
    ".pitlord",
    ".pytest_cache",
    ".ritual",
    ".vs",
    ".vscode",
    ".warlock",
    ".workingtrees",
    ".worktrees",
    "__pycache__",
    "artifacts",
    "bin",
    "build",
    "coverage",
    "dist",
    "generated",
    "node_modules",
    "obj",
    "out",
    "packages",
    "target",
    "temp",
    "tmp",
    "vendor",
];

pub struct Repository {
    pub name: String,
    pub sources: Vec<SourceFile>,
    pub manifests: Vec<ManifestFile>,
}

pub fn discover(root: &Path) -> Result<Repository, AdapterError> {
    let root = fs::canonicalize(root)
        .map_err(|error| AdapterError::new(format!("resolve repository: {error}")))?;
    if !root.is_dir() {
        return Err(AdapterError::new("repository path is not a directory"));
    }
    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AdapterError::new("repository path has no stable directory name"))?
        .to_owned();

    let mut sources = Vec::new();
    let mut manifests = Vec::new();
    walk(&root, &root, &mut sources, &mut manifests)?;
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    manifests.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(Repository {
        name,
        sources,
        manifests,
    })
}

fn walk(
    root: &Path,
    current: &Path,
    sources: &mut Vec<SourceFile>,
    manifests: &mut Vec<ManifestFile>,
) -> Result<(), AdapterError> {
    let mut entries = fs::read_dir(current)
        .map_err(AdapterError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AdapterError::from)?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let kind = entry.file_type().map_err(AdapterError::from)?;
        let path = entry.path();
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            let lower = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if !EXCLUDED.contains(&lower.as_str()) {
                walk(root, &path, sources, manifests)?;
            }
            continue;
        }
        if !kind.is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        let format = manifest_format(&name);
        let source = format.is_none()
            && matches!(
                path.extension()
                    .and_then(|value| value.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_deref(),
                Some("kt" | "kts")
            );
        if !source && format.is_none() {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .map_err(|error| AdapterError::new(error.to_string()))?
            .to_string_lossy()
            .replace('\\', "/");
        let content = fs::read(&path)
            .map_err(|error| AdapterError::new(format!("read {relative}: {error}")))?;
        if let Some(format) = format {
            manifests.push(ManifestFile {
                content,
                format: format.into(),
                path: relative,
            });
        } else {
            sources.push(SourceFile {
                content,
                path: relative,
            });
        }
    }
    Ok(())
}

fn manifest_format(name: &str) -> Option<&'static str> {
    match name.to_ascii_lowercase().as_str() {
        "build.gradle.kts" => Some("gradle-kotlin"),
        "build.gradle" => Some("gradle-groovy"),
        "pom.xml" => Some("maven"),
        _ => None,
    }
}

pub fn whole_file_span(path: &str, content: &[u8]) -> SourceSpan {
    let mut line = 1_u64;
    let mut column = 1_u64;
    let mut offset = 0;
    while offset < content.len() {
        if content[offset..].starts_with(b"\r\n") {
            offset += 2;
            line += 1;
            column = 1;
            continue;
        }
        if matches!(content[offset], b'\r' | b'\n') {
            offset += 1;
            line += 1;
            column = 1;
            continue;
        }
        let value = std::str::from_utf8(&content[offset..])
            .ok()
            .and_then(|value| value.chars().next())
            .unwrap_or('\u{FFFD}');
        offset += value.len_utf8().max(1);
        column += 1;
    }
    SourceSpan {
        end_column: column,
        end_line: line,
        path: path.into(),
        start_column: 1,
        start_line: 1,
    }
}

pub fn path_directory(path: &str) -> &str {
    path.rsplit_once('/')
        .map_or(".", |(directory, _)| directory)
}

pub fn base_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

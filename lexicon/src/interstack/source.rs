use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;

use crate::ScanExecutionError;

use super::model::{SourceFile, normalize_source_path};

const MAX_SOURCE_LOAD_WORKERS: usize = 16;

struct SourceDescriptor {
    path: PathBuf,
    relative: String,
    extension: String,
}

pub(crate) fn collect_source_files(
    root: &Path,
    allowed_languages: &HashSet<String>,
) -> Result<Vec<SourceFile>, ScanExecutionError> {
    let mut descriptors = Vec::new();
    walk(root, root, allowed_languages, &mut descriptors)?;
    descriptors.sort_by(|left, right| left.relative.cmp(&right.relative));
    load_sources(&descriptors)
}

fn walk(
    root: &Path,
    directory: &Path,
    allowed: &HashSet<String>,
    result: &mut Vec<SourceDescriptor>,
) -> Result<(), ScanExecutionError> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if path != root
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(ignored_directory)
            {
                continue;
            }
            walk(root, &path, allowed, result)?;
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .map_err(|error| ScanExecutionError::new(error.to_string()))?
            .to_string_lossy();
        let relative = normalize_source_path(&relative);
        if ignored_source_path(&relative) {
            continue;
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{}", value.to_ascii_lowercase()))
            .unwrap_or_default();
        let Some(language) = source_language(&extension) else {
            continue;
        };
        if !allowed.contains(language) || entry.metadata()?.len() > 2 * 1024 * 1024 {
            continue;
        }
        result.push(SourceDescriptor {
            path,
            relative,
            extension,
        });
    }
    Ok(())
}

fn load_sources(descriptors: &[SourceDescriptor]) -> Result<Vec<SourceFile>, ScanExecutionError> {
    if descriptors.is_empty() {
        return Ok(Vec::new());
    }
    let workers = descriptors
        .len()
        .min(
            thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1),
        )
        .min(MAX_SOURCE_LOAD_WORKERS)
        .max(1);
    let chunk_size = descriptors.len().div_ceil(workers);

    thread::scope(|scope| {
        let handles = descriptors
            .chunks(chunk_size)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(load_source)
                        .collect::<Result<Vec<_>, ScanExecutionError>>()
                })
            })
            .collect::<Vec<_>>();

        let mut result = Vec::with_capacity(descriptors.len());
        for handle in handles {
            for file in handle
                .join()
                .map_err(|_| ScanExecutionError::new("Interstack source loader panicked"))??
            {
                if let Some(file) = file {
                    result.push(file);
                }
            }
        }
        Ok(result)
    })
}

fn load_source(descriptor: &SourceDescriptor) -> Result<Option<SourceFile>, ScanExecutionError> {
    let data = fs::read(&descriptor.path)?;
    if data.contains(&0) {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&data).replace("\r\n", "\n");
    Ok(Some(SourceFile {
        path: descriptor.relative.clone(),
        extension: descriptor.extension.clone(),
        lines: text.split('\n').map(str::to_owned).collect(),
    }))
}

fn ignored_directory(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".lexicon"
            | ".arcana"
            | ".grimoire"
            | ".warlock"
            | ".worktrees"
            | ".workingtrees"
            | ".ddocs"
            | ".obsidian"
            | "node_modules"
            | "vendor"
            | "target"
            | "build"
            | "dist"
            | "coverage"
            | ".bundle"
    )
}

fn ignored_source_path(path: &str) -> bool {
    path.split('/').any(|part| {
        matches!(
            part.to_ascii_lowercase().as_str(),
            "test" | "tests" | "spec" | "fixtures"
        )
    })
}

fn source_language(extension: &str) -> Option<&'static str> {
    match extension {
        ".go" => Some("go"),
        ".rb" => Some("ruby"),
        ".gd" => Some("gdscript"),
        ".ts" | ".tsx" | ".js" | ".jsx" => Some("typescript"),
        ".py" => Some("python"),
        ".rs" => Some("rust"),
        _ => None,
    }
}

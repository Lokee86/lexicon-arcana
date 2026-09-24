use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::ScanExecutionError;

use super::model::{SourceFile, normalize_source_path};

pub(crate) fn collect_source_files(
    root: &Path,
    allowed_languages: &HashSet<String>,
) -> Result<Vec<SourceFile>, ScanExecutionError> {
    let mut result = Vec::new();
    walk(root, root, allowed_languages, &mut result)?;
    result.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(result)
}

fn walk(
    root: &Path,
    directory: &Path,
    allowed: &HashSet<String>,
    result: &mut Vec<SourceFile>,
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
        if !allowed.contains(language) {
            continue;
        }
        if entry.metadata()?.len() > 2 * 1024 * 1024 {
            continue;
        }
        let data = fs::read(&path)?;
        if data.contains(&0) {
            continue;
        }
        let text = String::from_utf8_lossy(&data).replace("\r\n", "\n");
        result.push(SourceFile {
            path: relative,
            extension,
            lines: text.split('\n').map(str::to_owned).collect(),
        });
    }
    Ok(())
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

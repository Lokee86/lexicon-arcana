use crate::AdapterError;
use std::{fs, path::Path};

const EXCLUDED_DIRECTORIES: &[&str] = &[
    ".arcana",
    ".bundle",
    ".cantrip",
    ".ddocs",
    ".git",
    ".godot",
    ".grimoire",
    ".homunculus",
    ".import",
    ".incubus",
    ".lexicon",
    ".next",
    ".pitlord",
    ".pytest_cache",
    ".ritual",
    ".venv",
    ".warlock",
    ".workingtrees",
    ".worktrees",
    "__pycache__",
    "bin",
    "build",
    "coverage",
    "dist",
    "node_modules",
    "obj",
    "target",
    "tmp",
    "vendor",
    "vendored",
    "venv",
];

const SOURCE_EXTENSIONS: &[&str] = &[
    "c", "cc", "cp", "cpp", "cxx", "c++", "h", "hh", "hpp", "hxx", "h++", "inc", "inl", "ipp",
    "tpp",
];

pub fn collect_sources(root: &Path) -> Result<Vec<String>, AdapterError> {
    let mut paths = Vec::new();
    walk(root, root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn walk(root: &Path, directory: &Path, paths: &mut Vec<String>) -> Result<(), AdapterError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| AdapterError::new(format!("scan repository: {error}")))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| AdapterError::new(format!("scan repository: {error}")))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| AdapterError::new(format!("scan repository: {error}")))?;
        if file_type.is_dir() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if !EXCLUDED_DIRECTORIES.contains(&name.as_str()) {
                walk(root, &path, paths)?;
            }
            continue;
        }
        if !file_type.is_file() || !is_source_path(&path) {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|error| AdapterError::new(format!("relativize source path: {error}")))?;
        paths.push(normalized(relative));
    }
    Ok(())
}

pub fn is_header_path(path: &str) -> bool {
    matches!(
        extension(path).as_str(),
        "h" | "h++" | "hh" | "hpp" | "hxx" | "inc" | "inl" | "ipp" | "tpp"
    )
}

pub fn extension(path: &str) -> String {
    if Path::new(path)
        .extension()
        .is_some_and(|value| value == "C")
    {
        return "C".into();
    }
    Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn is_source_path(path: &Path) -> bool {
    if path.extension().is_some_and(|value| value == "C") {
        return true;
    }
    path.extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|value| SOURCE_EXTENSIONS.contains(&value.as_str()))
}

fn normalized(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

use std::fs;
use std::path::{Path, PathBuf};

use rustpython_parser::{Parse, ast};

use crate::AdapterError;

use super::model::{Repository, SourceFile, SourceInput};

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
    ".bundle",
    ".eggs",
    ".mypy_cache",
    ".next",
    ".nox",
    ".pytest_cache",
    ".ruff_cache",
    ".tox",
    ".venv",
    "__pycache__",
    "build",
    "dist",
    "env",
    "node_modules",
    "site-packages",
    "target",
    "vendor",
    "venv",
];

pub fn discover(root: &Path) -> Result<Repository, AdapterError> {
    let root = fs::canonicalize(root)
        .map_err(|error| AdapterError::new(format!("open Python repository: {error}")))?;
    if !root.is_dir() {
        return Err(AdapterError::new("Python repository is not a directory"));
    }
    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("repository")
        .to_owned();

    let mut directories = vec![".".to_owned()];
    let mut paths = Vec::new();
    walk(&root, &root, &mut directories, &mut paths)?;
    directories.sort();
    paths.sort();

    let files = paths
        .into_iter()
        .map(|path| input(&root, &name, path))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Repository {
        root,
        name,
        directories,
        files,
    })
}

pub(super) fn load(input: &SourceInput) -> Result<SourceFile, AdapterError> {
    let bytes = fs::read(&input.path).map_err(AdapterError::from)?;
    match String::from_utf8(bytes.clone()) {
        Ok(source) => match ast::Suite::parse(&source, &input.relative) {
            Ok(suite) => Ok(SourceFile {
                path: input.path.clone(),
                relative: input.relative.clone(),
                module: input.module.clone(),
                bytes,
                source,
                suite: Some(suite),
                parse_error: None,
            }),
            Err(error) => Ok(SourceFile {
                path: input.path.clone(),
                relative: input.relative.clone(),
                module: input.module.clone(),
                bytes,
                source,
                suite: None,
                parse_error: Some(format!("{error}")),
            }),
        },
        Err(_) => Ok(SourceFile {
            path: input.path.clone(),
            relative: input.relative.clone(),
            module: input.module.clone(),
            bytes,
            source: String::new(),
            suite: None,
            parse_error: Some("UnicodeDecodeError".into()),
        }),
    }
}

fn input(root: &Path, repository: &str, path: PathBuf) -> Result<SourceInput, AdapterError> {
    let relative = relative(root, &path);
    let module = module_name(repository, &relative);
    let size = fs::metadata(&path).map_err(AdapterError::from)?.len();
    Ok(SourceInput {
        path,
        relative,
        module,
        size,
    })
}

fn walk(
    root: &Path,
    current: &Path,
    directories: &mut Vec<String>,
    files: &mut Vec<PathBuf>,
) -> Result<(), AdapterError> {
    let mut entries = fs::read_dir(current)
        .map_err(|error| AdapterError::new(format!("read Python source tree: {error}")))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AdapterError::from)?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let kind = entry.file_type().map_err(AdapterError::from)?;
        if kind.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if EXCLUDED.contains(&name.as_ref()) {
                continue;
            }
            directories.push(relative(root, &path));
            walk(root, &path, directories, files)?;
        } else if kind.is_file()
            && path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("py"))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn module_name(repository: &str, relative: &str) -> String {
    let mut parts = relative.split('/').map(str::to_owned).collect::<Vec<_>>();
    let leaf = parts.pop().unwrap_or_default();
    let stem = leaf.strip_suffix(".py").unwrap_or(&leaf);
    if stem == "__init__" {
        if parts.is_empty() {
            repository.to_owned()
        } else {
            parts.join(".")
        }
    } else {
        parts.push(stem.to_owned());
        parts.join(".")
    }
}

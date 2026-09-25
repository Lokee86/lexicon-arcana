use std::{fs, path::Path};

use crate::AdapterError;

use super::module_ownership;

const EXCLUDED_DIRECTORIES: &[&str] = &[
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
    "vendor",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceFile {
    pub path: String,
    pub content: Vec<u8>,
    pub module: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Module {
    pub root: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Inventory {
    pub repository: String,
    pub directories: Vec<String>,
    pub files: Vec<SourceFile>,
    pub modules: Vec<Module>,
}

impl Inventory {
    pub(crate) fn semantic_files(&self) -> Vec<String> {
        debug_assert!(
            self.files
                .iter()
                .all(|file| file.module == module_ownership::index(&self.modules, &file.path))
        );
        self.files.iter().map(|file| file.path.clone()).collect()
    }

    #[cfg(test)]
    pub(crate) fn module_for_file(&self, path: &str) -> Option<&Module> {
        self.files
            .iter()
            .find(|file| file.path == path)
            .and_then(|file| file.module)
            .and_then(|index| self.modules.get(index))
    }
}

pub(crate) fn discover(root: &Path) -> Result<Inventory, AdapterError> {
    let mut directories = Vec::new();
    let mut paths = Vec::new();
    walk(root, root, &mut directories, &mut paths)?;
    directories.sort();
    paths.sort();

    let mut files = paths
        .into_iter()
        .map(|path| read_source(root, path))
        .collect::<Result<Vec<_>, _>>()?;
    let modules = module_ownership::discover(&files)?;
    if modules.is_empty() {
        return Err(AdapterError::new(format!(
            "no go.mod files found under {}",
            root.display()
        )));
    }
    module_ownership::assign(&mut files, &modules);
    let repository = module_ownership::repository_identity(root, &modules);

    Ok(Inventory {
        repository,
        directories,
        files,
        modules,
    })
}

fn walk(
    root: &Path,
    directory: &Path,
    directories: &mut Vec<String>,
    files: &mut Vec<String>,
) -> Result<(), AdapterError> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| AdapterError::new(format!("scan repository: {error}")))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| AdapterError::new(format!("scan repository: {error}")))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| AdapterError::new(format!("scan repository: {error}")))?;
        if kind.is_dir() {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if EXCLUDED_DIRECTORIES.contains(&name.as_ref()) {
                continue;
            }
            directories.push(relative(root, &path)?);
            walk(root, &path, directories, files)?;
            continue;
        }
        if semantic_input(&path) {
            files.push(relative(root, &path)?);
        }
    }
    Ok(())
}

fn semantic_input(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "go.mod")
        || path.extension().is_some_and(|extension| extension == "go")
}

fn read_source(root: &Path, path: String) -> Result<SourceFile, AdapterError> {
    let absolute = root.join(path.replace('/', std::path::MAIN_SEPARATOR_STR));
    let content =
        fs::read(&absolute).map_err(|error| AdapterError::new(format!("read {path}: {error}")))?;
    Ok(SourceFile {
        path,
        content,
        module: None,
    })
}

fn relative(root: &Path, path: &Path) -> Result<String, AdapterError> {
    path.strip_prefix(root)
        .map(|value| value.to_string_lossy().replace('\\', "/"))
        .map_err(|error| AdapterError::new(format!("relativize source path: {error}")))
}

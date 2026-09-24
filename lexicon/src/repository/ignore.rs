use ignore::gitignore::{Gitignore, GitignoreBuilder};
use std::path::{Path, PathBuf};

use super::RepositoryError;

pub const IGNORE_FILE_NAME: &str = ".lexiconignore";

const IGNORED_DIRECTORIES: &[&str] = &[
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
    ".git",
    ".worktrees",
    ".workingtrees",
    ".astro",
    "node_modules",
    "vendor",
    "target",
    "dist",
    "build",
    ".venv",
    "venv",
    "__pycache__",
    ".pytest_cache",
];

pub struct IgnorePolicy {
    root: PathBuf,
    matcher: Option<Gitignore>,
}

impl IgnorePolicy {
    pub fn load(root: &Path) -> Result<Self, RepositoryError> {
        let root = absolute(root)?;
        let ignore_file = root.join(IGNORE_FILE_NAME);
        let matcher = if ignore_file.exists() {
            let mut builder = GitignoreBuilder::new(&root);
            if let Some(error) = builder.add(&ignore_file) {
                return Err(RepositoryError::new(error.to_string()));
            }
            Some(
                builder
                    .build()
                    .map_err(|error| RepositoryError::new(error.to_string()))?,
            )
        } else {
            None
        };
        Ok(Self { root, matcher })
    }

    pub fn ignored(&self, path: &Path, is_dir: bool) -> bool {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return true;
        };
        if relative.as_os_str().is_empty() {
            return false;
        }
        if contains_ignored_directory(relative) {
            return true;
        }
        let Some(matcher) = self.matcher.as_ref() else {
            return false;
        };
        let mut parent = relative.parent();
        while let Some(value) = parent {
            if value.as_os_str().is_empty() {
                break;
            }
            if matcher.matched(self.root.join(value), true).is_ignore() {
                return true;
            }
            parent = value.parent();
        }
        matcher.matched(path, is_dir).is_ignore()
    }
}

pub fn ignored_directory(name: &str) -> bool {
    IGNORED_DIRECTORIES.contains(&name)
}

fn contains_ignored_directory(path: &Path) -> bool {
    path.components()
        .any(|component| ignored_directory(component.as_os_str().to_string_lossy().as_ref()))
}

fn absolute(path: &Path) -> Result<PathBuf, RepositoryError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|current| current.join(path))
        .map_err(RepositoryError::from)
}

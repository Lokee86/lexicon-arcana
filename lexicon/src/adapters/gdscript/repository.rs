use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::AdapterError;

use super::facts::{Facts, add_file_facts, add_repository_facts, normalize_path};
use super::model::ParsedFile;
use super::parser::{parse_file, project_resource_path};

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
    ".godot",
    ".import",
    "build",
    "dist",
    "bin",
    "obj",
];

pub struct Repository {
    pub root: PathBuf,
    pub name: String,
    pub project_roots: Vec<String>,
    pub files: Vec<ParsedFile>,
}

pub fn load(path: &Path, facts: &mut Facts) -> Result<Repository, AdapterError> {
    let root = fs::canonicalize(path)
        .map_err(|error| AdapterError::new(format!("resolve repository: {error}")))?;
    if !root.is_dir() {
        return Err(AdapterError::new("repository path is not a directory"));
    }

    let (paths, directories, mut project_roots) = collect_sources(&root)?;
    if project_roots.is_empty() {
        project_roots.push(".".into());
    }
    project_roots.sort();
    project_roots.dedup();

    let name = root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("repository")
        .to_owned();
    add_repository_facts(facts, &name, &directories);

    let mut files = Vec::with_capacity(paths.len());
    for relative in paths {
        let content = fs::read(root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR)))
            .map_err(|error| AdapterError::new(format!("read {relative}: {error}")))?;
        let mut file = parse_file(&relative, &content)
            .map_err(|error| AdapterError::new(format!("parse {relative}: {error}")))?;
        file.project_root = nearest_project_root(&relative, &project_roots);
        facts
            .project_root_by_file_path
            .insert(normalize_path(&relative), file.project_root.clone());
        for declaration in &mut file.declarations {
            if !declaration.preload_path.is_empty() {
                declaration.preload_path =
                    project_resource_path(&file.project_root, &declaration.preload_path);
            }
        }
        add_file_facts(facts, &mut file, &directories);
        files.push(file);
    }

    Ok(Repository {
        root,
        name,
        project_roots,
        files,
    })
}

#[allow(clippy::type_complexity)]
fn collect_sources(root: &Path) -> Result<(Vec<String>, Vec<String>, Vec<String>), AdapterError> {
    let mut files = Vec::new();
    let mut project_roots = Vec::new();
    let mut all_directories = BTreeSet::from([".".to_owned()]);
    walk(
        root,
        root,
        &mut files,
        &mut project_roots,
        &mut all_directories,
    )?;
    files.sort();

    let mut needed = BTreeSet::from([".".to_owned()]);
    for file in &files {
        let mut directory = std::path::Path::new(file)
            .parent()
            .map(|value| value.to_string_lossy().replace('\\', "/"))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| ".".into());
        while directory != "." && !directory.is_empty() {
            needed.insert(directory.clone());
            directory = std::path::Path::new(&directory)
                .parent()
                .map(|value| value.to_string_lossy().replace('\\', "/"))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| ".".into());
        }
    }
    let directories = needed
        .into_iter()
        .filter(|directory| all_directories.contains(directory))
        .collect::<Vec<_>>();

    Ok((files, directories, project_roots))
}

fn walk(
    root: &Path,
    current: &Path,
    files: &mut Vec<String>,
    project_roots: &mut Vec<String>,
    directories: &mut BTreeSet<String>,
) -> Result<(), AdapterError> {
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
            directories.insert(relative(root, &path));
            walk(root, &path, files, project_roots, directories)?;
            continue;
        }
        if !kind.is_file() {
            continue;
        }
        let relative = relative(root, &path);
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case("project.godot")
        {
            let project_root = std::path::Path::new(&relative)
                .parent()
                .map(|value| value.to_string_lossy().replace('\\', "/"))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| ".".into());
            project_roots.push(project_root);
        }
        if path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("gd"))
        {
            files.push(relative);
        }
    }
    Ok(())
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub fn nearest_project_root(source_path: &str, project_roots: &[String]) -> String {
    let source = normalize_path(source_path);
    let mut best = ".".to_owned();
    for project_root in project_roots {
        let root = normalize_path(project_root);
        if (root == "." || source == root || source.starts_with(&format!("{root}/")))
            && root != "."
            && (best == "." || root.len() > best.len())
        {
            best = root;
        }
    }
    best
}

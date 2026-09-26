use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::languages::{for_path, owns_source};

pub fn build_analysis_scope(
    source_root: &Path,
    temporary_root: &Path,
    language: &str,
    context_files: &[String],
) -> Result<PathBuf, std::io::Error> {
    let repository = temporary_root.join(language).join("source");
    if let Some(parent) = repository.parent() {
        match fs::remove_dir_all(parent) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }

    let mut selected: BTreeSet<String> = context_files
        .iter()
        .map(|path| path.replace('\\', "/"))
        .collect();
    expand_semantic_units(source_root, language, &mut selected)?;
    copy_scope_tree(source_root, &repository, language, &selected)?;
    Ok(repository)
}

fn expand_semantic_units(
    root: &Path,
    language: &str,
    selected: &mut BTreeSet<String>,
) -> Result<(), std::io::Error> {
    let seeds: Vec<String> = selected.iter().cloned().collect();
    match language {
        "go" => {
            for path in seeds {
                if extension_eq(&path, "go") {
                    let directory = Path::new(&path).parent().unwrap_or(Path::new(""));
                    let module_root = nearest_config(root, directory, "go.mod");
                    include_go_module_sources(root, &module_root, selected)?;
                }
            }
        }
        "rust" => {
            for path in seeds {
                if extension_eq(&path, "rs") {
                    let directory = Path::new(&path).parent().unwrap_or(Path::new(""));
                    let crate_root = nearest_config(root, directory, "Cargo.toml");
                    include_tree_sources(root, &crate_root, "rs", selected)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn copy_scope_tree(
    source_root: &Path,
    destination_root: &Path,
    language: &str,
    selected: &BTreeSet<String>,
) -> Result<(), std::io::Error> {
    let mut stack = vec![source_root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                stack.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(source_root)
                .expect("walked path must remain beneath source root");
            let normalized = relative.to_string_lossy().replace('\\', "/");
            if selected.contains(&normalized) || language_config(language, &normalized) {
                copy_file(&path, &destination_root.join(relative))?;
            }
        }
    }
    Ok(())
}

fn include_go_module_sources(
    root: &Path,
    relative_root: &Path,
    selected: &mut BTreeSet<String>,
) -> Result<(), std::io::Error> {
    let start = root.join(relative_root);
    let mut stack = vec![start.clone()];
    while let Some(directory) = stack.pop() {
        let mut entries = fs::read_dir(&directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                let name = entry.file_name();
                if go_ignored_directory(name.to_string_lossy().as_ref())
                    || (path != start && path.join("go.mod").is_file())
                {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if path
                .extension()
                .is_some_and(|value| value.to_string_lossy().eq_ignore_ascii_case("go"))
            {
                let relative = path
                    .strip_prefix(root)
                    .expect("walked path must remain beneath root");
                selected.insert(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    Ok(())
}

fn go_ignored_directory(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".worktrees"
            | ".workingtrees"
            | ".ddocs"
            | ".lexicon"
            | ".arcana"
            | ".grimoire"
            | ".pitlord"
            | ".cantrip"
            | ".homunculus"
            | ".incubus"
            | ".ritual"
            | ".warlock"
            | "vendor"
    )
}

fn include_tree_sources(
    root: &Path,
    relative_root: &Path,
    extension: &str,
    selected: &mut BTreeSet<String>,
) -> Result<(), std::io::Error> {
    let start = root.join(relative_root);
    let mut stack = vec![start];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .is_some_and(|value| value.to_string_lossy().eq_ignore_ascii_case(extension))
            {
                let relative = path
                    .strip_prefix(root)
                    .expect("walked path must remain beneath root");
                selected.insert(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    Ok(())
}

fn nearest_config(root: &Path, relative_dir: &Path, name: &str) -> PathBuf {
    let mut current = relative_dir.to_path_buf();
    loop {
        if root.join(&current).join(name).is_file() {
            return current;
        }
        if !current.pop() {
            return PathBuf::from(".");
        }
    }
}

fn language_config(language: &str, path: &str) -> bool {
    !owns_source(language, path) && for_path(path).iter().any(|candidate| candidate == language)
}

fn extension_eq(path: &str, expected: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|value| value.to_string_lossy().eq_ignore_ascii_case(expected))
}

fn copy_file(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, destination)?;
    Ok(())
}

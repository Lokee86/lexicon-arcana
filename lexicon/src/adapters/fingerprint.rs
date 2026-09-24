use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::CONFIG_VERSION;
use crate::languages::lookup;

use super::{ADAPTER_SCHEMA_VERSION, AdapterError};

const IGNORED_DIRECTORIES: &[&str] = &[
    ".arcana",
    ".bundle",
    ".cantrip",
    ".ddocs",
    ".git",
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
    ".worktrees",
    ".workingtrees",
    "__pycache__",
    "bin",
    "build",
    "coverage",
    "dist",
    "log",
    "node_modules",
    "obj",
    "target",
    "test",
    "testdata",
    "tests",
    "tmp",
    "vendor",
    "venv",
];

pub fn adapter_fingerprint(root: &Path, language: &str) -> Result<String, AdapterError> {
    adapter_fingerprint_with_versions(
        root,
        language,
        ADAPTER_SCHEMA_VERSION,
        CONFIG_VERSION as u32,
    )
}

pub fn adapter_fingerprint_with_versions(
    root: &Path,
    language: &str,
    schema_version: u32,
    config_version: u32,
) -> Result<String, AdapterError> {
    let definition = lookup(language)
        .ok_or_else(|| AdapterError::new(format!("unsupported language {language:?}")))?;
    let adapter_root = root.join(&definition.directory);
    let paths = adapter_files(&adapter_root)
        .map_err(|error| AdapterError::new(format!("list {language} adapter files: {error}")))?;

    let mut hash = Sha256::new();
    write_field(&mut hash, b"lexicon:adapter-fingerprint:v1");
    write_field(&mut hash, definition.language.as_bytes());
    write_field(&mut hash, definition.directory.as_bytes());
    write_field(&mut hash, schema_version.to_string().as_bytes());
    write_field(&mut hash, config_version.to_string().as_bytes());
    for relative in paths {
        let data = fs::read(adapter_root.join(path_from_slash(&relative))).map_err(|error| {
            AdapterError::new(format!("read {language} adapter file {relative}: {error}"))
        })?;
        write_field(&mut hash, relative.as_bytes());
        write_field(&mut hash, &data);
    }
    Ok(format!("sha256:{:x}", hash.finalize()))
}

fn adapter_files(root: &Path) -> Result<Vec<String>, std::io::Error> {
    let mut paths = Vec::new();
    collect_files(root, root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_files(
    root: &Path,
    current: &Path,
    paths: &mut Vec<String>,
) -> Result<(), std::io::Error> {
    let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if IGNORED_DIRECTORIES.contains(&name.as_str()) {
                continue;
            }
            collect_files(root, &path, paths)?;
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if ignored_file(&name) {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        paths.push(relative);
    }
    Ok(())
}

fn ignored_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with("_test.go")
        || (lower.ends_with(".py") && (lower.starts_with("test_") || lower.ends_with("_test.py")))
        || lower.ends_with(".test.ts")
        || lower.ends_with(".test.tsx")
        || lower.ends_with(".spec.ts")
        || lower.ends_with(".spec.tsx")
}

fn write_field(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value);
}

fn path_from_slash(value: &str) -> PathBuf {
    value.split('/').collect()
}

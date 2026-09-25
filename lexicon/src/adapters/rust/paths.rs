use crate::adapters::rust::model::{Context, SourceFile};
use proc_macro2::Span;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(crate) fn span_value(span: Span, path: &str) -> Option<Value> {
    let start = span.start();
    let end = span.end();
    let mut value = Map::new();
    value.insert("end_column".into(), Value::Number((end.column + 1).into()));
    value.insert("end_line".into(), Value::Number(end.line.into()));
    value.insert("path".into(), Value::String(path.into()));
    value.insert(
        "start_column".into(),
        Value::Number((start.column + 1).into()),
    );
    value.insert("start_line".into(), Value::Number(start.line.into()));
    Some(Value::Object(value))
}

pub(crate) fn span_start(span: Span) -> (usize, usize) {
    let start = span.start();
    (start.line, start.column + 1)
}

pub(crate) fn relative_path(repo: &Path, path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    if let Ok(relative) = path.strip_prefix(repo) {
        return normalize_path(relative);
    }
    let repo_key = comparable_path(repo).trim_end_matches('/').to_string();
    let path_key = comparable_path(path);
    path_key
        .strip_prefix(&(repo_key + "/"))
        .map(str::to_string)
        .unwrap_or_else(|| normalize_path(path))
}

pub(crate) fn normalize_path(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    if value.is_empty() { ".".into() } else { value }
}

pub(crate) fn comparable_path(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    value
        .strip_prefix("//?/")
        .unwrap_or(&value)
        .to_ascii_lowercase()
}

pub(crate) fn source_path_for(context: &Context, candidate: &Path) -> Option<PathBuf> {
    let candidate = comparable_path(candidate);
    context
        .sources
        .keys()
        .find(|path| comparable_path(path) == candidate)
        .cloned()
}

pub(crate) fn resolve_module_file(
    sources: &BTreeMap<PathBuf, SourceFile>,
    source: &Path,
    name: &str,
) -> Option<PathBuf> {
    let parent = source.parent()?;
    let stem = source.file_stem()?.to_str()?;
    let mut bases = vec![parent.to_path_buf()];
    if stem != "lib" && stem != "main" && stem != "mod" {
        bases.push(parent.join(stem));
    }
    for base in bases {
        for candidate in [
            base.join(format!("{name}.rs")),
            base.join(name).join("mod.rs"),
        ] {
            if sources.contains_key(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

pub(crate) fn is_excluded(root: &Path, path: &Path) -> bool {
    let defaults = [
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
        "target",
        "node_modules",
        "vendor",
        "build",
        "dist",
        "out",
    ];
    path.strip_prefix(root)
        .ok()
        .into_iter()
        .flat_map(Path::components)
        .any(|component| {
            let value = component.as_os_str().to_string_lossy();
            defaults.iter().any(|default| *default == value)
        })
}

#[cfg(test)]
mod tests {
    use super::is_excluded;
    use std::path::Path;

    #[test]
    fn excludes_complete_warlock_state_directory_set_and_preserves_vendor() {
        let root = Path::new("/repository");
        for directory in [
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
        ] {
            assert!(
                is_excluded(root, &root.join(directory).join("ignored.rs")),
                "{directory}"
            );
        }
        assert!(is_excluded(root, &root.join("vendor").join("ignored.rs")));
        assert!(!is_excluded(root, &root.join("src").join("lib.rs")));
    }
}

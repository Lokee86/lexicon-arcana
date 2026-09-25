use crate::adapters::rust::model::SourceFile;
use crate::adapters::rust::paths::{is_excluded, relative_path};
use anyhow::{Context as AnyhowContext, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn parse_sources(repo: &Path) -> Result<BTreeMap<PathBuf, SourceFile>> {
    let mut paths = Vec::new();
    collect_rust_files(repo, repo, &mut paths)?;
    paths.sort();
    let mut sources = BTreeMap::new();
    for path in paths {
        let content = fs::read(&path)?;
        let syntax = syn::parse_file(
            std::str::from_utf8(&content)
                .with_context(|| format!("Rust source is not UTF-8: {}", path.display()))?,
        )
        .with_context(|| format!("cannot parse Rust source {}", path.display()))?;
        let relative = relative_path(repo, &path);
        sources.insert(
            path.clone(),
            SourceFile {
                path,
                relative,
                content,
                syntax,
            },
        );
    }
    Ok(sources)
}

fn collect_rust_files(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if !is_excluded(root, &path) {
                collect_rust_files(root, &path, output)?;
            }
        } else if file_type.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("rs")
        {
            output.push(path);
        }
    }
    Ok(())
}

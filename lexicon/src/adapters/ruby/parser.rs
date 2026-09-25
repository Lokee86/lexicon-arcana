#![allow(clippy::collapsible_if)]

use super::ADAPTER_VERSION;
use crate::{
    AdapterError, AdapterMode, AdapterRequest, Analysis, EdgeRecord, FACT_SCHEMA_VERSION,
    FactHeader, FactRecord, NodeRecord, SourceSpan, UnresolvedRecord, content_id, node_id,
};
use serde_json::json;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
use tree_sitter::{Node, Parser};

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
    "vendor",
    "node_modules",
    "target",
    "build",
    "dist",
    "tmp",
    "log",
    "coverage",
];

pub fn analyze(root: &Path, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
    let root = root
        .canonicalize()
        .map_err(|e| AdapterError::new(format!("open Ruby repository: {e}")))?;
    let repo = root
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("repository")
        .to_owned();
    let mut out = Vec::new();
    let mut files = Vec::new();
    walk(&root, &root, &mut files)?;
    files.sort();
    let repository_id = node_id("ruby", "repository", &repo);
    out.push(FactRecord::Node(NodeRecord {
        attributes: None,
        content_id: None,
        id: repository_id.clone(),
        kind: "repository".into(),
        name: repo.clone(),
        owner: None,
        path: ".".into(),
        qualified_name: repo.clone(),
        span: None,
    }));
    let mut directories = BTreeSet::new();
    for path in &files {
        if let Ok(relative) = path.strip_prefix(&root) {
            let mut current = String::new();
            for part in relative.parent().unwrap_or(Path::new(".")).components() {
                let part = part.as_os_str().to_string_lossy();
                if part == "." {
                    continue;
                }
                if current.is_empty() {
                    current = part.to_string();
                } else {
                    current.push('/');
                    current.push_str(&part);
                }
                directories.insert(current.clone());
            }
        }
    }
    for directory in directories {
        let id = node_id("ruby", "directory", &directory);
        out.push(FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: None,
            id: id.clone(),
            kind: "directory".into(),
            name: directory.rsplit('/').next().unwrap_or(&directory).into(),
            owner: None,
            path: directory.clone(),
            qualified_name: directory.clone(),
            span: None,
        }));
        out.push(FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner: None,
            relation: "contains".into(),
            source: repository_id.clone(),
            target: id,
            span: None,
        }));
    }
    for path in files {
        parse_file(&root, &repo, &path, &mut out)?;
    }
    super::dependencies::add(&root, &mut out);
    super::semantic::enrich(&root, &mut out);
    let changed =
        (request.mode == AdapterMode::Incremental).then(|| normalized(&request.changed_files));
    let removed =
        (request.mode == AdapterMode::Incremental).then(|| normalized(&request.removed_files));
    Ok(Analysis::new(
        FactHeader {
            adapter_version: ADAPTER_VERSION.into(),
            changed_files: changed,
            language: "ruby".into(),
            mode: request.mode.then_some("incremental".into()),
            record: "lexicon".into(),
            removed_files: removed,
            repository: repo,
            schema_version: FACT_SCHEMA_VERSION,
            shared_complete: (request.mode == AdapterMode::Incremental).then_some(true),
        },
        out,
    ))
}

fn walk(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), AdapterError> {
    let mut entries = fs::read_dir(dir)
        .map_err(AdapterError::from)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AdapterError::from)?;
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let n = e.file_name().to_string_lossy().to_string();
        if e.file_type().map_err(AdapterError::from)?.is_dir() {
            if !EXCLUDED.contains(&n.as_str()) {
                walk(root, &p, files)?;
            }
        } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("rb")) {
            files.push(p);
        }
    }
    let _ = root;
    Ok(())
}

fn parse_file(
    root: &Path,
    repo: &str,
    path: &Path,
    out: &mut Vec<FactRecord>,
) -> Result<(), AdapterError> {
    let bytes = fs::read(path).map_err(AdapterError::from)?;
    let source = String::from_utf8_lossy(&bytes);
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let last_line = source.lines().last().unwrap_or("");
    let span = SourceSpan {
        path: rel.clone(),
        start_line: 1,
        start_column: 1,
        end_line: source.lines().count().max(1) as u64,
        end_column: last_line.len() as u64,
    };
    let fid = node_id("ruby", "file", &rel);
    out.push(FactRecord::Node(NodeRecord {
        attributes: None,
        content_id: Some(content_id(&bytes)),
        id: fid.clone(),
        kind: "file".into(),
        name: rel.rsplit('/').next().unwrap_or(&rel).into(),
        owner: None,
        path: rel.clone(),
        qualified_name: rel.clone(),
        span: Some(span.clone()),
    }));
    if let Some(parent) = Path::new(&rel)
        .parent()
        .filter(|p| !p.as_os_str().is_empty() && *p != Path::new("."))
    {
        let parent = parent.to_string_lossy().replace('\\', "/");
        out.push(FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner: None,
            relation: "contains".into(),
            source: node_id("ruby", "directory", &parent),
            target: fid.clone(),
            span: None,
        }));
    }
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_ruby::LANGUAGE.into())
        .map_err(|e| AdapterError::new(format!("Ruby parser: {e}")))?;
    let tree = parser
        .parse(source.as_bytes(), None)
        .ok_or_else(|| AdapterError::new("Ruby parser returned no tree"))?;
    visit(
        tree.root_node(),
        source.as_bytes(),
        &rel,
        &fid,
        vec![repo.to_owned()],
        out,
    );
    Ok(())
}

include!("parser_visit.rs");
include!("parser_support.rs");

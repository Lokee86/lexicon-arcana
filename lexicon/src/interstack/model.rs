use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde_json::Value;

use crate::{NodeRecord, SourceSpan};

pub(crate) type Attributes = BTreeMap<String, Value>;

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub path: String,
    pub qualified_name: String,
    pub span: Option<SourceSpan>,
    pub attributes: Attributes,
}

impl From<NodeRecord> for Node {
    fn from(value: NodeRecord) -> Self {
        let attributes = value
            .attributes
            .and_then(|value| value.as_object().cloned())
            .map(|values| values.into_iter().collect())
            .unwrap_or_default();
        Self {
            id: value.id,
            kind: value.kind,
            name: value.name,
            path: normalize_source_path(&value.path),
            qualified_name: value.qualified_name,
            span: value.span,
            attributes,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Library {
    pub language: String,
    pub repository: String,
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone)]
pub(crate) struct SourceFile {
    pub path: String,
    pub extension: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SourceIndex {
    callables: HashMap<String, Vec<Node>>,
    files: HashMap<String, Node>,
    by_qname: HashMap<String, Vec<Node>>,
    by_name: HashMap<String, Vec<Node>>,
}

impl SourceIndex {
    pub(crate) fn new(libraries: &[Library]) -> Self {
        let mut index = Self::default();
        for library in libraries {
            for raw in &library.nodes {
                let mut node = raw.clone();
                node.path = normalize_source_path(&node.path);
                index
                    .by_qname
                    .entry(node.qualified_name.clone())
                    .or_default()
                    .push(node.clone());
                index
                    .by_name
                    .entry(node.name.clone())
                    .or_default()
                    .push(node.clone());
                if node.kind == "file" && !node.path.is_empty() {
                    index.files.insert(node.path.clone(), node.clone());
                }
                if is_callable(&node.kind) && !node.path.is_empty() && node.span.is_some() {
                    index
                        .callables
                        .entry(node.path.clone())
                        .or_default()
                        .push(node);
                }
            }
        }
        for values in index.callables.values_mut() {
            values.sort_by_key(|node| {
                let span = node.span.as_ref().expect("callable span");
                (span.start_line, span.start_column)
            });
        }
        index
    }

    pub(crate) fn owner_at(&self, path: &str, line: u64) -> Option<Node> {
        let path = normalize_source_path(path);
        let mut nearest = None;
        if let Some(candidates) = self.callables.get(&path) {
            for candidate in candidates {
                let span = candidate.span.as_ref().expect("callable span");
                if span.start_line > line {
                    break;
                }
                nearest = Some(candidate.clone());
                if span.start_line <= line && span.end_line >= line {
                    return Some(candidate.clone());
                }
            }
        }
        nearest.or_else(|| self.files.get(&path).cloned())
    }

    pub(crate) fn exact_qname(&self, name: &str) -> Option<Node> {
        let values = self.by_qname.get(name)?;
        (values.len() == 1).then(|| values[0].clone())
    }

    pub(crate) fn callable_by_name(&self, name: &str, preferred_path: &str) -> Option<Node> {
        let values = self.by_name.get(name)?;
        let component = component_for_path(preferred_path);
        let directory = Path::new(preferred_path)
            .parent()
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let mut best_score = -1_i32;
        let mut best = None;
        let mut tied = false;
        for candidate in values.iter().filter(|node| is_callable(&node.kind)) {
            let mut score = 0;
            if component_for_path(&candidate.path) == component {
                score += 4;
            }
            if !directory.is_empty() && candidate.path.starts_with(&(directory.clone() + "/")) {
                score += 2;
            }
            if score > best_score {
                best_score = score;
                best = Some(candidate.clone());
                tied = false;
            } else if score == best_score {
                tied = true;
            }
        }
        if best_score >= 0 && !tied { best } else { None }
    }
}

pub(crate) fn is_callable(kind: &str) -> bool {
    matches!(kind, "function" | "method" | "constructor" | "test")
}

pub(crate) fn normalize_source_path(value: &str) -> String {
    value.replace('\\', "/").trim_start_matches("./").to_owned()
}

pub(crate) fn component_for_path(value: &str) -> String {
    let normalized = normalize_source_path(value);
    let parts: Vec<_> = normalized.split('/').collect();
    if parts.len() >= 2 && parts[0] == "services" {
        return parts[..2].join("/");
    }
    parts
        .first()
        .copied()
        .filter(|value| !value.is_empty())
        .unwrap_or("repository")
        .to_owned()
}

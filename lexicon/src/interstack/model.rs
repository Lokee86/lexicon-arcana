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
pub(crate) struct SourceIndex<'a> {
    nodes: Vec<&'a Node>,
    callables: HashMap<String, Vec<usize>>,
    files: HashMap<String, usize>,
    by_qname: HashMap<String, Vec<usize>>,
    by_name: HashMap<String, Vec<usize>>,
}

impl<'a> SourceIndex<'a> {
    pub(crate) fn new(libraries: &'a [Library]) -> Self {
        let capacity = libraries.iter().map(|library| library.nodes.len()).sum();
        let mut index = Self {
            nodes: Vec::with_capacity(capacity),
            ..Self::default()
        };
        for library in libraries {
            for node in &library.nodes {
                let node_index = index.nodes.len();
                index
                    .by_qname
                    .entry(node.qualified_name.clone())
                    .or_default()
                    .push(node_index);
                index
                    .by_name
                    .entry(node.name.clone())
                    .or_default()
                    .push(node_index);
                let path = normalize_source_path(&node.path);
                if node.kind == "file" && !path.is_empty() {
                    index.files.insert(path.clone(), node_index);
                }
                if is_callable(&node.kind) && !path.is_empty() && node.span.is_some() {
                    index.callables.entry(path).or_default().push(node_index);
                }
                index.nodes.push(node);
            }
        }
        let nodes = &index.nodes;
        for values in index.callables.values_mut() {
            values.sort_by_key(|node_index| {
                let span = nodes[*node_index].span.as_ref().expect("callable span");
                (span.start_line, span.start_column)
            });
        }
        index
    }

    pub(crate) fn owner_at(&self, path: &str, line: u64) -> Option<Node> {
        let path = normalize_source_path(path);
        let mut nearest = None;
        if let Some(candidates) = self.callables.get(&path) {
            for node_index in candidates {
                let candidate = self.nodes[*node_index];
                let span = candidate.span.as_ref().expect("callable span");
                if span.start_line > line {
                    break;
                }
                nearest = Some(*node_index);
                if span.start_line <= line && span.end_line >= line {
                    return Some(candidate.clone());
                }
            }
        }
        nearest
            .or_else(|| self.files.get(&path).copied())
            .map(|node_index| self.nodes[node_index].clone())
    }

    pub(crate) fn exact_qname(&self, name: &str) -> Option<Node> {
        let values = self.by_qname.get(name)?;
        (values.len() == 1).then(|| self.nodes[values[0]].clone())
    }

    pub(crate) fn callable_by_name(&self, name: &str, preferred_path: &str) -> Option<Node> {
        let values = self.by_name.get(name)?;
        let component = component_for_path(preferred_path);
        let directory = Path::new(preferred_path)
            .parent()
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        let directory_prefix = (!directory.is_empty()).then(|| format!("{directory}/"));
        let mut best_score = -1_i32;
        let mut best = None;
        let mut tied = false;
        for node_index in values {
            let candidate = self.nodes[*node_index];
            if !is_callable(&candidate.kind) {
                continue;
            }
            let mut score = 0;
            if component_for_path(&candidate.path) == component {
                score += 4;
            }
            if directory_prefix
                .as_deref()
                .is_some_and(|prefix| candidate.path.starts_with(prefix))
            {
                score += 2;
            }
            if score > best_score {
                best_score = score;
                best = Some(*node_index);
                tied = false;
            } else if score == best_score {
                tied = true;
            }
        }
        if best_score >= 0 && !tied {
            best.map(|node_index| self.nodes[node_index].clone())
        } else {
            None
        }
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

use super::{
    model::{IncludeObservation, SourceFile},
    syntax::{node_text, span},
};
use crate::node_id;
use std::{collections::HashMap, path::Path};
use tree_sitter::Node;

#[derive(Debug)]
pub struct FileIndex<'a> {
    by_path: HashMap<&'a str, &'a SourceFile>,
    by_base_name: HashMap<String, Vec<&'a SourceFile>>,
}

impl<'a> FileIndex<'a> {
    pub fn new(files: &'a [SourceFile]) -> Self {
        let mut by_path = HashMap::with_capacity(files.len());
        let mut by_base_name = HashMap::<String, Vec<&SourceFile>>::new();
        for file in files {
            by_path.insert(file.path.as_str(), file);
            by_base_name
                .entry(base_name(&file.path).to_ascii_lowercase())
                .or_default()
                .push(file);
        }
        for values in by_base_name.values_mut() {
            values.sort_by(|left, right| left.path.cmp(&right.path));
        }
        Self {
            by_path,
            by_base_name,
        }
    }

    pub fn local_target(&self, observation: &IncludeObservation) -> Option<&'a SourceFile> {
        let target = lexical_path(&observation.target);
        if let Some(file) = self.by_path.get(target.as_str()) {
            return Some(*file);
        }

        let parent = parent_path(&observation.path);
        let relative = lexical_path(&join_path(parent, &target));
        if !relative.starts_with("../")
            && let Some(file) = self.by_path.get(relative.as_str())
        {
            return Some(*file);
        }

        let matches = self
            .by_base_name
            .get(&base_name(&target).to_ascii_lowercase())?;
        (matches.len() == 1).then_some(matches[0])
    }
}

pub fn extract(file: &mut SourceFile, node: Node<'_>, source: &[u8]) {
    let Some(path_node) = node.child_by_field_name("path") else {
        return;
    };
    let expression = node_text(path_node, source).to_owned();
    let target = strip_include_target(&expression);
    if target.is_empty() {
        return;
    }
    let canonical = format!("{}::include::{target}::{}", file.path, node.start_byte());
    file.includes.push(IncludeObservation {
        id: node_id("c-family", "import", &canonical),
        module_id: node_id("c-family", "module", &file.path),
        path: file.path.clone(),
        target,
        expression,
        system: path_node.kind() == "system_lib_string",
        span: span(&file.path, node),
    });
}

fn strip_include_target(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_start_matches('"')
        .trim_end_matches('"')
        .replace('\\', "/")
}

fn lexical_path(value: &str) -> String {
    let normalized = value.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|last| *last != "..") => {
                parts.pop();
            }
            ".." => parts.push(".."),
            value => parts.push(value),
        }
    }
    parts.join("/")
}

fn parent_path(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

fn join_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.into()
    } else {
        format!("{parent}/{child}")
    }
}

fn base_name(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_else(|| path.rsplit('/').next().unwrap_or(path))
}

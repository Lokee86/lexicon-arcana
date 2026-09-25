use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::{
    EdgeRecord, FactRecord, NodeRecord, SourceSpan, UnresolvedRecord, content_id, node_id,
};

use super::model::{Declaration, ParsedFile};

#[derive(Debug)]
pub struct Facts {
    pub nodes: BTreeMap<String, NodeRecord>,
    edges: BTreeMap<String, EdgeRecord>,
    unresolved: BTreeMap<String, UnresolvedRecord>,

    pub module_by_path: BTreeMap<String, String>,
    pub class_by_name: BTreeMap<String, Vec<String>>,
    pub class_by_file_and_name: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    pub method_by_owner_id: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    pub static_method_by_owner_id: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    pub type_by_owner_id: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    pub owner_by_function_id: BTreeMap<String, String>,
    pub declaration_by_id: BTreeMap<String, Declaration>,
    pub declared_member_by_owner: BTreeMap<String, BTreeSet<String>>,
    pub declared_local_by_function: BTreeMap<String, BTreeSet<String>>,
    pub member_value_by_owner: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    pub local_value_by_function: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    pub parent_by_owner_id: BTreeMap<String, Vec<String>>,
    pub external_parent_by_owner_id: BTreeSet<String>,
    pub script_owner_by_path: BTreeMap<String, String>,
    pub script_owner_candidates_by_path: BTreeMap<String, Vec<String>>,
    pub project_root_by_file_path: BTreeMap<String, String>,
    pub autoload_owner_by_project_name: BTreeMap<String, BTreeMap<String, String>>,
    pub preload_alias_by_file_and_name: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

impl Facts {
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            unresolved: BTreeMap::new(),
            module_by_path: BTreeMap::new(),
            class_by_name: BTreeMap::new(),
            class_by_file_and_name: BTreeMap::new(),
            method_by_owner_id: BTreeMap::new(),
            static_method_by_owner_id: BTreeMap::new(),
            type_by_owner_id: BTreeMap::new(),
            owner_by_function_id: BTreeMap::new(),
            declaration_by_id: BTreeMap::new(),
            declared_member_by_owner: BTreeMap::new(),
            declared_local_by_function: BTreeMap::new(),
            member_value_by_owner: BTreeMap::new(),
            local_value_by_function: BTreeMap::new(),
            parent_by_owner_id: BTreeMap::new(),
            external_parent_by_owner_id: BTreeSet::new(),
            script_owner_by_path: BTreeMap::new(),
            script_owner_candidates_by_path: BTreeMap::new(),
            project_root_by_file_path: BTreeMap::new(),
            autoload_owner_by_project_name: BTreeMap::new(),
            preload_alias_by_file_and_name: BTreeMap::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_node(
        &mut self,
        kind: &str,
        name: &str,
        path: &str,
        qualified_name: &str,
        identity: &str,
        span: Option<SourceSpan>,
        content: Option<&[u8]>,
        attributes: Option<Value>,
    ) -> String {
        let id = node_id("gdscript", kind, identity);
        self.nodes.entry(id.clone()).or_insert_with(|| NodeRecord {
            attributes,
            content_id: content.map(content_id),
            id: id.clone(),
            kind: kind.into(),
            name: name.into(),
            owner: None,
            path: path.into(),
            qualified_name: qualified_name.into(),
            span,
        });
        id
    }

    pub fn add_edge(
        &mut self,
        source: &str,
        target: &str,
        relation: &str,
        span: Option<SourceSpan>,
        attributes: Option<Value>,
    ) {
        let record = EdgeRecord {
            attributes,
            owner: None,
            relation: relation.into(),
            source: source.into(),
            span,
            target: target.into(),
        };
        self.edges.entry(edge_key(&record)).or_insert(record);
    }

    pub fn add_dataflow_edge(
        &mut self,
        source: &str,
        target: &str,
        relation: &str,
        span: Option<SourceSpan>,
    ) {
        self.add_edge(source, target, relation, span, None);
    }

    pub fn add_unresolved(
        &mut self,
        source: &str,
        relation: &str,
        expression: &str,
        reason: &str,
        span: Option<SourceSpan>,
        candidate_name: Option<String>,
    ) {
        let record = UnresolvedRecord {
            attributes: None,
            candidate_name,
            candidate_namespace: None,
            expression: expression.into(),
            owner: None,
            reason: reason.into(),
            relation: relation.into(),
            source: source.into(),
            span,
        };
        self.unresolved
            .entry(unresolved_key(&record))
            .or_insert(record);
    }

    pub fn index_declaration(&mut self, declaration: &Declaration) {
        self.declaration_by_id
            .insert(declaration.node_id.clone(), declaration.clone());

        if matches!(declaration.kind.as_str(), "variable" | "constant") {
            if !declaration.owner_function.is_empty() {
                self.declared_local_by_function
                    .entry(declaration.owner_function.clone())
                    .or_default()
                    .insert(declaration.name.clone());
                self.local_value_by_function
                    .entry(declaration.owner_function.clone())
                    .or_default()
                    .entry(declaration.name.clone())
                    .or_default()
                    .push(declaration.node_id.clone());
            } else {
                self.declared_member_by_owner
                    .entry(declaration.owner_id.clone())
                    .or_default()
                    .insert(declaration.name.clone());
                self.member_value_by_owner
                    .entry(declaration.owner_id.clone())
                    .or_default()
                    .entry(declaration.name.clone())
                    .or_default()
                    .push(declaration.node_id.clone());
            }
        }

        if declaration.kind == "type" && declaration.keyword != "class_name" {
            self.type_by_owner_id
                .entry(declaration.owner_id.clone())
                .or_default()
                .entry(declaration.name.clone())
                .or_default()
                .push(declaration.node_id.clone());
            return;
        }

        if declaration.kind != "function" {
            return;
        }
        self.method_by_owner_id
            .entry(declaration.owner_id.clone())
            .or_default()
            .entry(declaration.name.clone())
            .or_default()
            .push(declaration.node_id.clone());
        self.owner_by_function_id
            .insert(declaration.node_id.clone(), declaration.owner_id.clone());
        if declaration.is_static {
            self.static_method_by_owner_id
                .entry(declaration.owner_id.clone())
                .or_default()
                .entry(declaration.name.clone())
                .or_default()
                .push(declaration.node_id.clone());
        }
    }

    pub fn index_parent(&mut self, source: &str, target: &str) {
        let parents = self.parent_by_owner_id.entry(source.into()).or_default();
        if !parents.iter().any(|value| value == target) {
            parents.push(target.into());
            parents.sort();
        }
    }

    pub fn index_class(&mut self, path: &str, name: &str, id: &str) {
        self.class_by_file_and_name
            .entry(normalize_path(path))
            .or_default()
            .entry(name.into())
            .or_default()
            .push(id.into());
    }

    pub fn index_preload_alias(&mut self, path: &str, name: &str, target_path: &str) {
        self.preload_alias_by_file_and_name
            .entry(normalize_path(path))
            .or_default()
            .entry(name.into())
            .or_default()
            .push(normalize_path(target_path));
    }

    pub fn local_value_ids(&self, function_id: &str, name: &str) -> &[String] {
        self.local_value_by_function
            .get(function_id)
            .and_then(|values| values.get(name))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn member_value_ids(&self, owner_id: &str, name: &str) -> &[String] {
        self.member_value_by_owner
            .get(owner_id)
            .and_then(|values| values.get(name))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn into_records(self) -> Vec<FactRecord> {
        self.nodes
            .into_values()
            .map(FactRecord::Node)
            .chain(self.edges.into_values().map(FactRecord::Edge))
            .chain(self.unresolved.into_values().map(FactRecord::Unresolved))
            .collect()
    }
}

pub fn add_repository_facts(facts: &mut Facts, repository_name: &str, directories: &[String]) {
    let repository_id = facts.add_node(
        "repository",
        repository_name,
        ".",
        repository_name,
        repository_name,
        None,
        None,
        None,
    );
    let root_id = facts.add_node(
        "directory",
        repository_name,
        ".",
        ".",
        ".",
        None,
        None,
        None,
    );
    facts.add_edge(&repository_id, &root_id, "contains", None, None);

    let mut ids = BTreeMap::from([(".".to_owned(), root_id)]);
    for directory in directories.iter().filter(|value| value.as_str() != ".") {
        let name = std::path::Path::new(directory)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(directory);
        let id = facts.add_node(
            "directory",
            name,
            directory,
            directory,
            directory,
            None,
            None,
            None,
        );
        ids.insert(directory.clone(), id);
    }
    for directory in directories.iter().filter(|value| value.as_str() != ".") {
        let parent = std::path::Path::new(directory)
            .parent()
            .map(|value| value.to_string_lossy().replace('\\', "/"))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| ".".into());
        if let (Some(parent), Some(child)) = (ids.get(&parent), ids.get(directory)) {
            facts.add_edge(parent, child, "contains", None, None);
        }
    }
}

pub fn add_file_facts(facts: &mut Facts, file: &mut ParsedFile, directories: &[String]) {
    let file_id = facts.add_node(
        "file",
        std::path::Path::new(&file.path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(&file.path),
        &file.path,
        &file.path,
        &file.path,
        None,
        Some(&file.content),
        None,
    );
    let module_id = facts.add_node(
        "module",
        std::path::Path::new(&file.path)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(&file.path),
        &file.path,
        &file.path,
        &file.path,
        None,
        None,
        None,
    );
    file.module_id = module_id.clone();
    facts
        .module_by_path
        .insert(file.path.clone(), module_id.clone());

    let directory = std::path::Path::new(&file.path)
        .parent()
        .map(|value| value.to_string_lossy().replace('\\', "/"))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| ".".into());
    if directories.iter().any(|value| value == &directory) {
        let directory_id = node_id("gdscript", "directory", &directory);
        facts.add_edge(&directory_id, &file_id, "contains", None, None);
    }
    facts.add_edge(&file_id, &module_id, "contains", None, None);
}

pub fn normalize_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    if parts.is_empty() {
        ".".into()
    } else {
        parts.join("/")
    }
}

fn edge_key(value: &EdgeRecord) -> String {
    format!(
        "{}\0{}\0{}\0{:?}\0{}",
        value.source,
        value.target,
        value.relation,
        value.span,
        value
            .attributes
            .as_ref()
            .map_or(String::new(), Value::to_string)
    )
}

fn unresolved_key(value: &UnresolvedRecord) -> String {
    format!(
        "{}\0{}\0{}\0{}\0{:?}\0{}",
        value.source,
        value.relation,
        value.expression,
        value.reason,
        value.span,
        value.candidate_name.as_deref().unwrap_or_default()
    )
}

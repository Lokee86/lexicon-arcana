use std::fs;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::json;

use super::facts::Facts;
use super::parser::project_resource_path;
use super::repository::Repository;
use crate::AdapterError;

static RESOURCE_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"res://([A-Za-z0-9_./-]+)").unwrap());

pub fn process_project(repository: &Repository, facts: &mut Facts) -> Result<(), AdapterError> {
    let repository_id = crate::node_id("gdscript", "repository", &repository.name);
    for project_root in &repository.project_roots {
        let path = repository
            .root
            .join(project_root.replace('/', std::path::MAIN_SEPARATOR_STR))
            .join("project.godot");
        let source = match fs::read_to_string(path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(AdapterError::from(error)),
        };
        let mut section = String::new();
        for raw in source.lines() {
            let line = raw.split(';').next().unwrap_or("").trim();
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].into();
                continue;
            }
            if line.is_empty() {
                continue;
            }
            let category = match section.as_str() {
                "editor_plugins" => "plugin",
                "autoload" => "autoload",
                _ => "local",
            };
            let source_name = format!("project.godot:{section}");
            for capture in RESOURCE_PATH.captures_iter(line) {
                let resource = project_resource_path(project_root, &capture[1]);
                let target = dependency_target(facts, &resource);
                facts.add_edge(
                    &repository_id,
                    &target,
                    "depends-on",
                    None,
                    Some(dependency_attributes(category, &source_name, true)),
                );
            }
        }
    }
    Ok(())
}

pub fn dependency_attributes(category: &str, source: &str, path: bool) -> serde_json::Value {
    json!({
        "build": false,
        "category": category,
        "constraint": "",
        "dev": false,
        "optional": false,
        "path": path,
        "peer": false,
        "source": source,
    })
}

pub fn dependency_target(facts: &mut Facts, target_path: &str) -> String {
    let target_path = super::facts::normalize_path(target_path);
    if let Some(id) = facts.module_by_path.get(&target_path) {
        return id.clone();
    }
    let identity = format!("dependency:gdscript:{target_path}");
    facts.add_node(
        "module",
        std::path::Path::new(&target_path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(&target_path),
        &format!("@dependencies/godot/{target_path}"),
        &identity,
        &identity,
        None,
        None,
        Some(json!({"dependency": true, "ecosystem": "godot"})),
    )
}

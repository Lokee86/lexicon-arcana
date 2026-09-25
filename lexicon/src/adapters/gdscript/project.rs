use std::fs;

use super::facts::{Facts, normalize_path};
use super::parser::{normalize_import_path, project_resource_path};
use super::repository::Repository;
use crate::AdapterError;

pub fn process_autoloads(repository: &Repository, facts: &mut Facts) -> Result<(), AdapterError> {
    for project_root in &repository.project_roots {
        let path = repository
            .root
            .join(project_root.replace('/', std::path::MAIN_SEPARATOR_STR))
            .join("project.godot");
        let source = match fs::read_to_string(&path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(AdapterError::from(error)),
        };
        let mut section = "";
        for raw in source.lines() {
            let line = raw.trim();
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim();
                continue;
            }
            if section != "autoload" || line.is_empty() || line.starts_with(';') {
                continue;
            }
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            let name = name.trim();
            let value = value.trim();
            if name.is_empty()
                || value.len() < 2
                || !value.starts_with('"')
                || !value.ends_with('"')
            {
                continue;
            }
            let value = value[1..value.len() - 1].trim_start_matches('*');
            let Some(path) = normalize_import_path(value) else {
                continue;
            };
            let path = project_resource_path(project_root, &path);
            if let Some(owner) = facts.script_owner_by_path.get(&path).cloned() {
                facts
                    .autoload_owner_by_project_name
                    .entry(normalize_path(project_root))
                    .or_default()
                    .insert(name.into(), owner);
            }
        }
    }
    Ok(())
}

use std::fs;

use regex::Regex;
use serde_json::json;

use super::facts::Facts;
use super::model::Repository;
use super::resolve::bindings::{resolve_module_name, resolve_relative_module};

pub fn add_dependency_facts(repository: &Repository, facts: &mut Facts) {
    let Some(repository_id) = facts
        .nodes
        .values()
        .find(|node| node.kind == "repository")
        .map(|node| node.id.clone())
    else {
        return;
    };

    let (mut manifest, has_project_dependencies) = pyproject_dependencies(repository);
    if !has_project_dependencies {
        manifest.extend(requirements_dependencies(repository));
    }

    for dependency in manifest {
        let target = dependency_target(
            facts,
            if dependency.local_path.is_empty() {
                &dependency.name
            } else {
                &dependency.local_path
            },
            &dependency.local_path,
        );
        facts.add_edge(
            &repository_id,
            &target,
            "depends-on",
            None,
            Some(attributes(
                &dependency.category,
                &dependency.source,
                &dependency.constraint,
                !dependency.local_path.is_empty(),
            )),
        );
    }

    let imports = facts.imports.clone();
    for info in imports {
        let requested = resolve_relative_module(&info);
        if requested.is_empty() {
            continue;
        }
        let Some(module_name) = resolve_module_name(facts, &requested, &info.module_name) else {
            continue;
        };
        let Some(source) = facts.modules.get(&info.module_name).cloned() else {
            continue;
        };
        let Some(target) = facts.modules.get(&module_name).cloned() else {
            continue;
        };
        facts.add_edge(
            &source,
            &target,
            "depends-on",
            None,
            Some(attributes("local", &requested, "", true)),
        );
    }
}

#[derive(Debug)]
struct Dependency {
    name: String,
    constraint: String,
    source: String,
    category: String,
    local_path: String,
}

fn pyproject_dependencies(repository: &Repository) -> (Vec<Dependency>, bool) {
    let path = repository.root.join("pyproject.toml");
    let Ok(source) = fs::read_to_string(path) else {
        return (Vec::new(), false);
    };
    let Ok(value) = source.parse::<toml::Value>() else {
        return (Vec::new(), false);
    };
    let Some(project) = value.get("project").and_then(toml::Value::as_table) else {
        return (Vec::new(), false);
    };

    let has_dependencies = project.contains_key("dependencies");
    let mut result = Vec::new();
    if let Some(values) = project.get("dependencies").and_then(toml::Value::as_array) {
        for value in values {
            let Some(value) = value.as_str() else {
                continue;
            };
            let Some((name, constraint)) = literal_requirement(value) else {
                continue;
            };
            result.push(Dependency {
                name,
                constraint,
                source: "pyproject.toml:project.dependencies".into(),
                category: "runtime".into(),
                local_path: String::new(),
            });
        }
    }

    if let Some(optional) = project
        .get("optional-dependencies")
        .and_then(toml::Value::as_table)
    {
        let mut extras = optional.iter().collect::<Vec<_>>();
        extras.sort_by_key(|(name, _)| *name);
        for (extra, values) in extras {
            let Some(values) = values.as_array() else {
                continue;
            };
            for value in values {
                let Some(value) = value.as_str() else {
                    continue;
                };
                let Some((name, constraint)) = literal_requirement(value) else {
                    continue;
                };
                result.push(Dependency {
                    name,
                    constraint,
                    source: format!("pyproject.toml:project.optional-dependencies.{extra}"),
                    category: if extra == "test" {
                        "test".into()
                    } else {
                        "optional".into()
                    },
                    local_path: String::new(),
                });
            }
        }
    }
    (result, has_dependencies)
}

fn requirements_dependencies(repository: &Repository) -> Vec<Dependency> {
    let Ok(entries) = fs::read_dir(&repository.root) else {
        return Vec::new();
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("requirements") && name.ends_with(".txt"))
        })
        .collect::<Vec<_>>();
    paths.sort();

    let mut result = Vec::new();
    for path in paths {
        let Some(filename) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        for raw in source.lines() {
            let value = raw.split('#').next().unwrap_or("").trim();
            if value.is_empty()
                || value.starts_with("-r")
                || value.starts_with("--")
                || value.starts_with("-c")
                || value.starts_with("-f")
                || value.starts_with("git+")
                || value.starts_with("http:")
                || value.starts_with("https:")
            {
                continue;
            }
            if let Some(local) = value
                .strip_prefix("-e ")
                .or_else(|| value.strip_prefix("--editable "))
            {
                let local = local.trim().replace('\\', "/");
                if !local.is_empty()
                    && !local.starts_with('/')
                    && local != ".."
                    && !local.starts_with("../")
                {
                    result.push(Dependency {
                        name: local.trim_start_matches("./").to_owned(),
                        constraint: String::new(),
                        source: format!("{filename}:editable"),
                        category: category(filename),
                        local_path: local.trim_start_matches("./").to_owned(),
                    });
                }
                continue;
            }
            let Some((name, constraint)) = literal_requirement(value) else {
                continue;
            };
            result.push(Dependency {
                name,
                constraint,
                source: format!("{filename}:requirement"),
                category: category(filename),
                local_path: String::new(),
            });
        }
    }
    result
}

fn literal_requirement(value: &str) -> Option<(String, String)> {
    let value = value.trim();
    if value.is_empty()
        || value.starts_with('-')
        || value.starts_with("git+")
        || value.starts_with("http:")
        || value.starts_with("https:")
    {
        return None;
    }
    let pattern = Regex::new(r"^([A-Za-z0-9][A-Za-z0-9._-]*)(?:\s*(.*))?$").ok()?;
    let captures = pattern.captures(value)?;
    let name = captures.get(1)?.as_str().to_owned();
    let constraint = captures.get(2).map_or("", |value| value.as_str()).trim();
    if constraint.matches('[').count() != constraint.matches(']').count() {
        return None;
    }
    Some((name, constraint.to_owned()))
}

fn category(filename: &str) -> String {
    if filename.contains("requirements-test") {
        "test".into()
    } else if filename.contains("requirements-dev") {
        "development".into()
    } else {
        "runtime".into()
    }
}

fn dependency_target(facts: &mut Facts, name: &str, local_path: &str) -> String {
    let normalized = name.replace('\\', "/");
    let identity = format!("dependency:python:{normalized}");
    let path = if local_path.is_empty() {
        format!("@dependencies/python/{normalized}")
    } else {
        local_path.to_owned()
    };
    facts.add_node(
        "module",
        &normalized,
        &path,
        &identity,
        Some(&identity),
        None,
        Some(json!({"dependency": true, "ecosystem": "python"})),
        None,
    )
}

fn attributes(category: &str, source: &str, constraint: &str, path: bool) -> serde_json::Value {
    json!({
        "build": category == "build",
        "category": category,
        "constraint": constraint,
        "dev": matches!(category, "development" | "test"),
        "optional": category == "optional",
        "path": path,
        "peer": category == "peer",
        "source": source,
    })
}

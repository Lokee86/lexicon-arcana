use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use serde_json::json;

use crate::{AdapterError, AdapterRequest, EdgeRecord, FactRecord, NodeRecord};

use super::{
    discovery::{Inventory, Module, SourceFile},
    identities,
    observations::{DeclarationKind, Observation},
    semantic_fact_index::FactIndex,
    semantic_identity_policy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Dependency {
    name: String,
    constraint: String,
    source: &'static str,
    replacement: String,
    category: &'static str,
    path: bool,
}

#[derive(Debug, Clone)]
struct PackageCandidate {
    namespace: String,
    name: String,
    id: String,
}

pub(crate) fn add(
    request: &AdapterRequest,
    inventory: &Inventory,
    semantic: &[Observation],
    records: &mut Vec<FactRecord>,
    index: &mut FactIndex,
) -> Result<(), AdapterError> {
    let repository_id = index.node_id(&identities::repository(&inventory.repository))?;
    let repository_root = fs::canonicalize(&request.repository).map_err(|error| {
        AdapterError::new(format!(
            "cannot resolve repository {}: {error}",
            request.repository.display()
        ))
    })?;

    for module in &inventory.modules {
        let manifest = manifest_file(inventory, module)?;
        let content = manifest.manifest_content.as_deref().ok_or_else(|| {
            AdapterError::new(format!("missing retained manifest {}", manifest.path))
        })?;
        for mut dependency in parse_dependencies(content)? {
            let mut target_name = dependency.name.clone();
            let mut local_path = String::new();
            if dependency.replacement.starts_with("./") || dependency.replacement.starts_with("../")
            {
                let module_root = if module.root == "." {
                    repository_root.clone()
                } else {
                    repository_root.join(module.root.replace('/', std::path::MAIN_SEPARATOR_STR))
                };
                let candidate = lexical_clean(
                    &module_root.join(
                        dependency
                            .replacement
                            .replace('/', std::path::MAIN_SEPARATOR_STR),
                    ),
                );
                if let Ok(relative) = candidate.strip_prefix(&repository_root) {
                    local_path = if relative.as_os_str().is_empty() {
                        ".lexicon-repository".into()
                    } else {
                        slash_path(relative)?
                    };
                    if let Ok(replacement_module) = read_module_path(&candidate) {
                        target_name = replacement_module;
                    }
                    dependency.path = true;
                }
            }

            let target_id =
                dependency_node(&target_name, dependency.path, &local_path, records, index)?;
            push_dependency_edge(
                records,
                index,
                repository_id.clone(),
                target_id,
                None,
                dependency_attributes(
                    dependency.category,
                    dependency.source,
                    &dependency.constraint,
                    dependency.path,
                ),
            );
        }
    }

    add_local_imports(inventory, semantic, records, index)?;
    Ok(())
}

fn add_local_imports(
    inventory: &Inventory,
    semantic: &[Observation],
    records: &mut Vec<FactRecord>,
    index: &mut FactIndex,
) -> Result<(), AdapterError> {
    let packages = package_candidates(semantic, index)?;
    for record in semantic {
        let Observation::Declaration {
            kind: DeclarationKind::Import,
            metadata,
            ..
        } = record
        else {
            continue;
        };
        let import_path = metadata
            .get("import_path")
            .map(String::as_str)
            .unwrap_or("");
        if import_path.is_empty()
            || !semantic_identity_policy::is_internal_namespace(&inventory.modules, import_path)
        {
            continue;
        }
        let alias = metadata
            .get("import_alias")
            .map(String::as_str)
            .unwrap_or_else(|| import_path.rsplit('/').next().unwrap_or(import_path));
        if matches!(alias, "_" | ".") {
            continue;
        }
        let source_identity = metadata
            .get("container")
            .map(String::as_str)
            .ok_or_else(|| AdapterError::new("Go import declaration is missing container"))?;
        let source = index.semantic_node_id(source_identity)?;
        let Some(target) = package_for_namespace(&packages, &inventory.modules, import_path) else {
            continue;
        };
        let owner = index.node_owner(&source);
        push_dependency_edge(
            records,
            index,
            source,
            target,
            owner,
            dependency_attributes("local", import_path, "", true),
        );
    }
    Ok(())
}

fn manifest_file<'a>(
    inventory: &'a Inventory,
    module: &Module,
) -> Result<&'a SourceFile, AdapterError> {
    let path = if module.root == "." {
        "go.mod".to_owned()
    } else {
        format!("{}/go.mod", module.root)
    };
    inventory
        .files
        .iter()
        .find(|file| file.path == path)
        .ok_or_else(|| AdapterError::new(format!("missing discovered Go manifest {path}")))
}

fn parse_dependencies(content: &[u8]) -> Result<Vec<Dependency>, AdapterError> {
    let text = String::from_utf8_lossy(content);
    let mut result = Vec::new();
    let mut section = "";
    for raw in text.lines() {
        let line = raw
            .split_once("//")
            .map(|(head, _)| head)
            .unwrap_or(raw)
            .trim();
        if line.is_empty() {
            continue;
        }
        if line.ends_with('(') {
            section = line.trim_end_matches('(').trim();
            continue;
        }
        if line == ")" {
            section = "";
            continue;
        }
        if section == "require" || line.starts_with("require ") {
            let fields = line
                .trim_start_matches("require ")
                .split_whitespace()
                .collect::<Vec<_>>();
            if fields.len() >= 2 {
                result.push(Dependency {
                    name: fields[0].into(),
                    constraint: fields[1].into(),
                    source: "go.mod:require",
                    replacement: String::new(),
                    category: "runtime",
                    path: false,
                });
            }
            continue;
        }
        if section == "replace" || line.starts_with("replace ") {
            let text = line.trim_start_matches("replace ").trim();
            let Some((left, right)) = text.split_once("=>") else {
                continue;
            };
            let left = left.split_whitespace().collect::<Vec<_>>();
            let right = right.split_whitespace().collect::<Vec<_>>();
            if left.is_empty() || right.is_empty() {
                continue;
            }
            let constraint = left.get(1).copied().unwrap_or("");
            let mut replacement = right[0].to_owned();
            if let Some(version) = right.get(1)
                && !replacement.starts_with('.')
            {
                replacement.push('@');
                replacement.push_str(version);
            }
            result.push(Dependency {
                name: left[0].into(),
                constraint: constraint.into(),
                source: "go.mod:replace",
                replacement,
                category: "runtime",
                path: false,
            });
        }
    }
    Ok(result)
}

fn dependency_node(
    name: &str,
    local: bool,
    local_path: &str,
    records: &mut Vec<FactRecord>,
    index: &mut FactIndex,
) -> Result<String, AdapterError> {
    let identity = format!("package:dependency:go:{name}");
    let id = index.node_id_for_kind(&identity, "module")?;
    let path = if local {
        local_path.to_owned()
    } else {
        format!("@dependencies/go/{}", name.replace('\\', "/"))
    };
    index.push_node(
        records,
        NodeRecord {
            attributes: Some(json!({"dependency": true, "ecosystem": "go"})),
            content_id: None,
            id: id.clone(),
            kind: "module".into(),
            name: name.into(),
            owner: None,
            path: path.clone(),
            qualified_name: format!("{path}::{name}"),
            span: None,
        },
    );
    Ok(id)
}

fn dependency_attributes(
    category: &str,
    source: &str,
    constraint: &str,
    local: bool,
) -> serde_json::Value {
    json!({
        "build": category == "build",
        "category": category,
        "constraint": constraint,
        "dev": category == "development",
        "optional": category == "optional",
        "path": local,
        "peer": category == "peer",
        "source": source,
    })
}

fn push_dependency_edge(
    records: &mut Vec<FactRecord>,
    index: &mut FactIndex,
    source: String,
    target: String,
    owner: Option<String>,
    attributes: serde_json::Value,
) {
    index.push_edge(
        records,
        EdgeRecord {
            attributes: Some(attributes),
            owner,
            relation: "depends-on".into(),
            source,
            span: None,
            target,
        },
    );
}

fn package_candidates(
    records: &[Observation],
    index: &mut FactIndex,
) -> Result<Vec<PackageCandidate>, AdapterError> {
    let mut result = Vec::new();
    for record in records {
        let Observation::Declaration {
            semantic_key,
            kind: DeclarationKind::Package,
            name,
            ..
        } = record
        else {
            continue;
        };
        let identity = index.canonical_semantic_identity(semantic_key)?;
        let body = identity.strip_prefix("package:").ok_or_else(|| {
            AdapterError::new(format!("invalid Go package identity {identity:?}"))
        })?;
        let (namespace, _) = body.rsplit_once(':').ok_or_else(|| {
            AdapterError::new(format!("invalid Go package identity {identity:?}"))
        })?;
        result.push(PackageCandidate {
            namespace: namespace.into(),
            name: name.clone(),
            id: index.node_id_for_kind(&identity, "module")?,
        });
    }
    Ok(result)
}

fn package_for_namespace(
    packages: &[PackageCandidate],
    modules: &[Module],
    namespace: &str,
) -> Option<String> {
    let namespace = semantic_identity_policy::canonical_namespace(modules, namespace);
    let expected_name = namespace.rsplit('/').next().unwrap_or(&namespace);
    packages
        .iter()
        .filter(|package| package.namespace == namespace)
        .map(|package| {
            let mut score = 0;
            if !package.name.ends_with("_test") {
                score += 1;
            }
            if package.name == expected_name {
                score += 2;
            }
            (score, package.id.clone())
        })
        .max_by(|left, right| left.0.cmp(&right.0).then_with(|| right.1.cmp(&left.1)))
        .map(|(_, id)| id)
}

fn read_module_path(root: &Path) -> Result<String, AdapterError> {
    let content = fs::read(root.join("go.mod"))
        .map_err(|error| AdapterError::new(format!("open go.mod: {error}")))?;
    let text = String::from_utf8_lossy(&content);
    for line in text.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() >= 2 && fields[0] == "module" {
            return Ok(fields[1].into());
        }
    }
    Err(AdapterError::new("no module directive"))
}

fn lexical_clean(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            _ => result.push(component.as_os_str()),
        }
    }
    result
}

fn slash_path(path: &Path) -> Result<String, AdapterError> {
    let value = path
        .to_str()
        .ok_or_else(|| AdapterError::new("dependency path is not valid UTF-8"))?
        .replace('\\', "/");
    if value.is_empty() || value == "." || value == ".." || value.starts_with("../") {
        return Err(AdapterError::new(format!(
            "invalid repository path {value:?}"
        )));
    }
    Ok(value)
}

use std::collections::BTreeSet;

use super::facts::{Facts, normalize_path};
use super::model::ParsedFile;
use super::parser::{is_builtin, normalize_import_path, project_resource_path};

pub fn process_file(facts: &mut Facts, file: &ParsedFile) {
    for declaration in &file.declarations {
        if declaration.extends.is_empty() {
            continue;
        }
        let source = if declaration.kind == "extends" || declaration.node_id.is_empty() {
            file.script_owner_id.as_str()
        } else {
            declaration.node_id.as_str()
        };

        if let Some(path) = normalize_import_path(&declaration.extends) {
            let path = project_resource_path(&file.project_root, &path);
            let target = facts
                .script_owner_by_path
                .get(&path)
                .or_else(|| facts.module_by_path.get(&path))
                .cloned();
            if let Some(target) = target {
                facts.add_edge(
                    source,
                    &target,
                    "extends",
                    Some(declaration.span.clone()),
                    None,
                );
                facts.index_parent(source, &target);
            } else {
                facts.add_unresolved(
                    source,
                    "extends",
                    &declaration.extends,
                    "missing-target",
                    Some(declaration.span.clone()),
                    None,
                );
            }
            continue;
        }

        let name = declaration.extends.trim();
        if let Some(owners) = preload_type_owners(facts, &file.path, name) {
            match owners.as_slice() {
                [target] if target != source => {
                    facts.add_edge(
                        source,
                        target,
                        "extends",
                        Some(declaration.span.clone()),
                        None,
                    );
                    facts.index_parent(source, target);
                }
                values => facts.add_unresolved(
                    source,
                    "extends",
                    &declaration.extends,
                    if values.len() > 1 {
                        "ambiguous-target"
                    } else {
                        "missing-target"
                    },
                    Some(declaration.span.clone()),
                    Some(name.into()),
                ),
            }
            continue;
        }

        let same_file = facts
            .class_by_file_and_name
            .get(&normalize_path(&file.path))
            .and_then(|values| values.get(name))
            .cloned()
            .unwrap_or_default();
        if same_file.len() == 1 && same_file[0] != source {
            facts.add_edge(
                source,
                &same_file[0],
                "extends",
                Some(declaration.span.clone()),
                None,
            );
            facts.index_parent(source, &same_file[0]);
            continue;
        }

        let global = facts.class_by_name.get(name).cloned().unwrap_or_default();
        if global.len() == 1 && global[0] != source {
            facts.add_edge(
                source,
                &global[0],
                "extends",
                Some(declaration.span.clone()),
                None,
            );
            facts.index_parent(source, &global[0]);
        } else if global.len() > 1 || same_file.len() > 1 {
            facts.add_unresolved(
                source,
                "extends",
                &declaration.extends,
                "ambiguous-target",
                Some(declaration.span.clone()),
                Some(name.into()),
            );
        } else {
            facts.external_parent_by_owner_id.insert(source.into());
            facts.add_unresolved(
                source,
                "extends",
                &declaration.extends,
                if is_builtin(name) {
                    "builtin-target"
                } else {
                    "external-target"
                },
                Some(declaration.span.clone()),
                Some(name.into()),
            );
        }
    }
}

pub fn process_overrides(facts: &mut Facts) {
    let function_ids = facts
        .owner_by_function_id
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for method_id in function_ids {
        let Some(method) = facts.declaration_by_id.get(&method_id).cloned() else {
            continue;
        };
        if method.owner_class_id.is_empty() {
            continue;
        }
        let parents = facts
            .parent_by_owner_id
            .get(&method.owner_class_id)
            .cloned()
            .unwrap_or_default();
        for parent in parents {
            for target in inherited_methods(facts, &parent, &method.name, &mut BTreeSet::new()) {
                if target != method_id {
                    facts.add_edge(
                        &method_id,
                        &target,
                        "overrides",
                        Some(method.span.clone()),
                        None,
                    );
                }
            }
        }
    }
}

fn inherited_methods(
    facts: &Facts,
    owner: &str,
    name: &str,
    seen: &mut BTreeSet<String>,
) -> Vec<String> {
    if owner.is_empty() || !seen.insert(owner.into()) {
        return Vec::new();
    }
    if let Some(methods) = facts
        .method_by_owner_id
        .get(owner)
        .and_then(|values| values.get(name))
        && !methods.is_empty()
    {
        return unique(methods.clone());
    }
    let mut result = Vec::new();
    if let Some(parents) = facts.parent_by_owner_id.get(owner) {
        for parent in parents {
            result.extend(inherited_methods(facts, parent, name, seen));
        }
    }
    unique(result)
}

fn preload_type_owners(facts: &Facts, source_path: &str, name: &str) -> Option<Vec<String>> {
    let mut parts = name.split('.');
    let alias = parts.next()?;
    let paths = facts
        .preload_alias_by_file_and_name
        .get(&normalize_path(source_path))
        .and_then(|values| values.get(alias))?;
    let mut owners = paths
        .iter()
        .filter_map(|path| {
            facts
                .script_owner_by_path
                .get(&normalize_path(path))
                .cloned()
        })
        .collect::<Vec<_>>();
    owners = unique(owners);

    for nested in parts {
        owners = unique(
            owners
                .iter()
                .flat_map(|owner| {
                    facts
                        .type_by_owner_id
                        .get(owner)
                        .and_then(|values| values.get(nested))
                        .cloned()
                        .unwrap_or_default()
                })
                .collect(),
        );
        if owners.is_empty() {
            break;
        }
    }
    Some(owners)
}

pub fn unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

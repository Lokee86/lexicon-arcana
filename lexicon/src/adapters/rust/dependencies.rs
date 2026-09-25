use crate::adapters::rust::model::Context;
use cargo_metadata::{DependencyKind, Metadata};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) fn dependency_attributes(
    category: &str,
    source: &str,
    constraint: &str,
    optional: bool,
    path: bool,
) -> BTreeMap<String, Value> {
    let mut attributes = BTreeMap::new();
    attributes.insert("build".into(), Value::Bool(category == "build"));
    attributes.insert("category".into(), Value::String(category.into()));
    attributes.insert("constraint".into(), Value::String(constraint.into()));
    attributes.insert(
        "dev".into(),
        Value::Bool(category == "development" || category == "test"),
    );
    attributes.insert("optional".into(), Value::Bool(optional));
    attributes.insert("path".into(), Value::Bool(path));
    attributes.insert("peer".into(), Value::Bool(false));
    attributes.insert("source".into(), Value::String(source.into()));
    attributes
}

fn category(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Development => "development",
        DependencyKind::Build => "build",
        _ => "runtime",
    }
}

pub(crate) fn add_dependencies(context: &mut Context, metadata: &[Metadata]) {
    let mut package_targets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for crate_context in &context.crates {
        if let Some((package, _)) = crate_context.qn.split_once("::") {
            package_targets
                .entry(package.to_string())
                .or_default()
                .push(crate_context.node_id.clone());
        }
    }
    for values in package_targets.values_mut() {
        values.sort();
        values.dedup();
    }

    for package in crate::adapters::rust::discovery::metadata_packages(metadata) {
        let Some(source_id) = package_targets
            .get(&package.name)
            .and_then(|values| values.first())
        else {
            continue;
        };
        let mut dependencies = package.dependencies.clone();
        dependencies.sort_by(|left, right| left.name.cmp(&right.name));
        for dependency in dependencies {
            let category = category(dependency.kind);
            let constraint = dependency.req.to_string();
            let local_path = dependency
                .path
                .as_ref()
                .map(|value| {
                    let path = Path::new(value.as_std_path());
                    path.strip_prefix(&context.repo)
                        .map(crate::adapters::rust::paths::normalize_path)
                        .unwrap_or_else(|_| format!("path:{}", dependency.name))
                })
                .unwrap_or_default();
            let is_local = !local_path.is_empty();
            let source = if is_local {
                local_path.clone()
            } else {
                dependency
                    .source
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "crates.io".into())
            };
            let mut attributes = dependency_attributes(
                category,
                &source,
                &constraint,
                dependency.optional,
                is_local,
            );
            if let Some(target) = &dependency.target {
                attributes.insert("target".into(), Value::String(target.to_string()));
            }
            let targets = if is_local {
                package_targets
                    .get(&dependency.name)
                    .cloned()
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            if targets.is_empty() {
                let normalized = dependency
                    .rename
                    .clone()
                    .unwrap_or_else(|| dependency.name.clone());
                let identity = format!("dependency:rust:{normalized}:{constraint}:{source}");
                let target = context.facts.add_node(
                    "rust",
                    "module",
                    &identity,
                    &normalized,
                    &format!("@dependencies/rust/{normalized}"),
                    &identity,
                    None,
                    None,
                    attributes.clone(),
                );
                context.facts.add_edge_with_attributes(
                    source_id,
                    &target,
                    "depends-on",
                    None,
                    attributes,
                );
            } else {
                for target in targets {
                    context.facts.add_edge_with_attributes(
                        source_id,
                        &target,
                        "depends-on",
                        None,
                        attributes.clone(),
                    );
                }
            }
        }
    }
}

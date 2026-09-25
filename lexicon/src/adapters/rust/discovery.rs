use crate::adapters::rust::contract::{content_id, stable_id};
use crate::adapters::rust::model::Context;
use crate::adapters::rust::paths::{
    comparable_path, is_excluded, normalize_path, relative_path, source_path_for,
};
use anyhow::{Context as AnyhowContext, Result, bail};
use cargo_metadata::{Metadata, MetadataCommand, Package};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn load_metadata(repo: &Path) -> Result<Vec<Metadata>> {
    let mut manifests = Vec::new();
    collect_manifests(repo, repo, &mut manifests)?;
    manifests.sort_by(|left, right| {
        left.components()
            .count()
            .cmp(&right.components().count())
            .then_with(|| left.cmp(right))
    });
    if manifests.is_empty() {
        bail!("no Cargo.toml found under {}", repo.display());
    }

    let mut covered_manifests = BTreeSet::new();
    let mut projects = Vec::new();
    for manifest in manifests {
        let canonical = fs::canonicalize(&manifest).unwrap_or_else(|_| manifest.clone());
        if covered_manifests.contains(&comparable_path(&canonical)) {
            continue;
        }
        let metadata = MetadataCommand::new()
            .manifest_path(&manifest)
            .no_deps()
            .exec()
            .with_context(|| format!("cargo metadata failed for {}", manifest.display()))?;
        for package in &metadata.packages {
            let package_manifest = PathBuf::from(package.manifest_path.as_std_path());
            let package_manifest = fs::canonicalize(&package_manifest).unwrap_or(package_manifest);
            covered_manifests.insert(comparable_path(&package_manifest));
        }
        projects.push(metadata);
    }
    Ok(projects)
}

fn collect_manifests(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if !is_excluded(root, &path) {
                collect_manifests(root, &path, output)?;
            }
        } else if file_type.is_file()
            && path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml")
        {
            output.push(path);
        }
    }
    Ok(())
}

pub(crate) fn metadata_packages(metadata: &[Metadata]) -> Vec<Package> {
    let mut packages = BTreeMap::new();
    for project in metadata {
        for package in &project.packages {
            let manifest = PathBuf::from(package.manifest_path.as_std_path());
            packages
                .entry(comparable_path(&manifest))
                .or_insert_with(|| package.clone());
        }
    }
    packages.into_values().collect()
}

fn is_analyzable_target_kind(kind: &str) -> bool {
    matches!(
        kind,
        "lib"
            | "rlib"
            | "dylib"
            | "cdylib"
            | "staticlib"
            | "proc-macro"
            | "bin"
            | "example"
            | "test"
            | "bench"
            | "custom-build"
    )
}

pub(crate) fn repository_identity(repo: &Path, metadata: &[Metadata]) -> String {
    let packages = metadata_packages(metadata);
    if packages.len() == 1 {
        return packages[0].name.clone();
    }
    repo.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("repository")
        .to_string()
}

pub(crate) fn add_repository_and_files(context: &mut Context) {
    let repo_id = context.facts.add_node(
        "rust",
        "repository",
        &context.repository,
        &context.repository,
        ".",
        &context.repository,
        None,
        None,
        Default::default(),
    );
    let mut directories = BTreeSet::new();
    for source in context.sources.values() {
        let mut current = Path::new(source.relative.as_str()).parent();
        while let Some(path) = current {
            if path.as_os_str().is_empty() || path == Path::new(".") {
                break;
            }
            directories.insert(path.to_path_buf());
            current = path.parent();
        }
    }
    for directory in directories {
        let relative = normalize_path(&directory);
        let name = directory
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(&relative);
        let id = context.facts.add_node(
            "rust",
            "directory",
            &relative,
            name,
            &relative,
            &relative,
            None,
            None,
            Default::default(),
        );
        let parent = directory
            .parent()
            .filter(|path| !path.as_os_str().is_empty());
        let parent_id = parent
            .map(|path| stable_id("rust", "directory", &normalize_path(path)))
            .unwrap_or_else(|| repo_id.clone());
        context.facts.add_edge(&parent_id, &id, "contains", None);
    }
    for source in context.sources.values() {
        let file_id = context.facts.add_node(
            "rust",
            "file",
            &source.relative,
            Path::new(&source.relative)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(&source.relative),
            &source.relative,
            &source.relative,
            Some(content_id(&source.content)),
            None,
            Default::default(),
        );
        let parent = Path::new(&source.relative).parent();
        let parent_id = parent
            .filter(|path| !path.as_os_str().is_empty())
            .map(|path| stable_id("rust", "directory", &normalize_path(path)))
            .unwrap_or(repo_id.clone());
        context
            .facts
            .add_edge(&parent_id, &file_id, "contains", None);
    }
}

pub(crate) fn add_crates(context: &mut Context, metadata: &[Metadata]) {
    let mut packages = metadata_packages(metadata);
    packages.sort_by(|left, right| {
        left.name.cmp(&right.name).then_with(|| {
            left.manifest_path
                .as_str()
                .cmp(right.manifest_path.as_str())
        })
    });
    for package in packages {
        let manifest_path = PathBuf::from(package.manifest_path.as_std_path());
        let package_root = fs::canonicalize(&manifest_path)
            .unwrap_or_else(|_| manifest_path.clone())
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf();
        let mut targets = package.targets.clone();
        targets.sort_by(|left, right| left.name.cmp(&right.name));
        for target in targets {
            if !target
                .kind
                .iter()
                .any(|kind| is_analyzable_target_kind(kind.to_string().as_str()))
            {
                continue;
            }
            let root_path = PathBuf::from(target.src_path.as_std_path());
            let root = fs::canonicalize(&root_path).unwrap_or(root_path);
            let Some(root) = source_path_for(context, &root) else {
                continue;
            };
            let qn = format!("{}::{}", package.name, target.name);
            let path = relative_path(&context.repo, package_root.join("Cargo.toml"));
            let mut attributes = std::collections::BTreeMap::new();
            attributes.insert("package".into(), Value::String(package.name.clone()));
            attributes.insert(
                "target_kind".into(),
                Value::String(
                    target
                        .kind
                        .first()
                        .map(|kind| kind.to_string().to_lowercase())
                        .unwrap_or_else(|| "crate".into()),
                ),
            );
            let node_id = context.facts.add_node(
                "rust",
                "module",
                &format!("crate:{qn}"),
                &target.name,
                &path,
                &qn,
                None,
                None,
                attributes,
            );
            context.modules.insert(qn.clone(), node_id.clone());
            let external_crates = package
                .dependencies
                .iter()
                .map(|dependency| {
                    dependency
                        .rename
                        .clone()
                        .unwrap_or_else(|| dependency.name.replace('-', "_"))
                })
                .collect();
            context
                .crates
                .push(crate::adapters::rust::model::CrateContext {
                    qn,
                    node_id,
                    root,
                    package_root: package_root.clone(),
                    external_crates,
                });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::is_analyzable_target_kind;

    #[test]
    fn accepts_all_cargo_rust_code_target_kinds() {
        for kind in [
            "lib",
            "rlib",
            "dylib",
            "cdylib",
            "staticlib",
            "proc-macro",
            "bin",
            "example",
            "test",
            "bench",
            "custom-build",
        ] {
            assert!(is_analyzable_target_kind(kind), "rejected {kind}");
        }
        assert!(!is_analyzable_target_kind("unknown"));
    }
}

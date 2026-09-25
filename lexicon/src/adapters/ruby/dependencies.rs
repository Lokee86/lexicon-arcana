use crate::{EdgeRecord, FactRecord, NodeRecord, node_id};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};

const EXCLUDED: &[&str] = &[
    ".git",
    ".worktrees",
    ".workingtrees",
    ".ddocs",
    ".lexicon",
    ".arcana",
    ".grimoire",
    ".pitlord",
    ".cantrip",
    ".homunculus",
    ".incubus",
    ".ritual",
    ".warlock",
    ".bundle",
    "vendor",
    "target",
    "node_modules",
    "build",
    "dist",
    "tmp",
    "log",
    "coverage",
];

pub fn add(root: &Path, records: &mut Vec<FactRecord>) {
    let mut files = Vec::new();
    collect(root, root, &mut files);
    files.sort();

    for path in files {
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let relative = rel(root, &path);
        let source_id = ensure_file(records, &path, &relative);

        if path.file_name().is_some_and(|n| n == "Gemfile")
            || path.extension().is_some_and(|e| e == "gemspec")
        {
            for (name, constraint, category) in gems(&source, &relative) {
                let normalized = name.replace('/', "::");
                let target = dependency_node(records, &normalized);
                add_dependency(
                    records,
                    &source_id,
                    &target,
                    &constraint,
                    &category,
                    &relative,
                    None,
                );
            }
        }

        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("rb"))
        {
            for target in require_relatives(&source) {
                if let Some(local_path) = resolve_local(&relative, &target) {
                    let absolute = root.join(&local_path);
                    if absolute.exists() {
                        let target_id = ensure_file(records, &absolute, &local_path);
                        add_dependency(
                            records,
                            &source_id,
                            &target_id,
                            "",
                            "local",
                            &relative,
                            Some(&local_path),
                        );
                    }
                }
            }
        }
    }
}

fn ensure_file(records: &mut Vec<FactRecord>, path: &Path, relative: &str) -> String {
    let id = node_id("ruby", "file", relative);
    if !records
        .iter()
        .any(|r| matches!(r, FactRecord::Node(n) if n.id == id))
    {
        records.push(FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: fs::read(path).ok().map(|b| crate::content_id(&b)),
            id: id.clone(),
            kind: "file".into(),
            name: relative.rsplit('/').next().unwrap_or(relative).into(),
            owner: None,
            path: relative.into(),
            qualified_name: relative.into(),
            span: None,
        }));
    }
    id
}

fn dependency_node(records: &mut Vec<FactRecord>, name: &str) -> String {
    let qualified = format!("dependency:ruby:{name}");
    let id = node_id("ruby", "module", &qualified);
    if !records
        .iter()
        .any(|r| matches!(r, FactRecord::Node(n) if n.id == id))
    {
        records.push(FactRecord::Node(NodeRecord {
            attributes: Some(json!({"dependency":true,"ecosystem":"ruby"})),
            content_id: None,
            id: id.clone(),
            kind: "module".into(),
            name: name.into(),
            owner: None,
            path: format!("@dependencies/ruby/{name}"),
            qualified_name: qualified,
            span: None,
        }));
    }
    id
}

fn add_dependency(
    records: &mut Vec<FactRecord>,
    source: &str,
    target: &str,
    constraint: &str,
    category: &str,
    manifest_path: &str,
    local_path: Option<&str>,
) {
    if records.iter().any(|r| {
        matches!(r, FactRecord::Edge(e)
        if e.source == source && e.target == target && e.relation == "depends-on")
    }) {
        return;
    }
    let mut attrs = json!({
        "constraint": constraint,
        "category": category,
        "dev": category == "development",
        "path": manifest_path,
    });
    if let Some(path) = local_path {
        attrs["path"] = json!(path);
    }
    records.push(FactRecord::Edge(EdgeRecord {
        attributes: Some(attrs),
        owner: None,
        relation: "depends-on".into(),
        source: source.into(),
        target: target.into(),
        span: None,
    }));
}

fn gems(source: &str, path: &str) -> Vec<(String, String, String)> {
    let mut result = Vec::new();
    let mut in_dev_group = false;
    for raw in source.lines() {
        let line = raw.trim();
        if line.starts_with("group ") && line.contains("development") {
            in_dev_group = true;
            continue;
        }
        if in_dev_group && line == "end" {
            in_dev_group = false;
            continue;
        }
        let category = if path.ends_with(".gemspec") && line.contains("add_development_dependency")
        {
            "development"
        } else if line.contains("gem ") || line.contains("add_dependency") {
            if in_dev_group {
                "development"
            } else {
                "runtime"
            }
        } else {
            continue;
        };
        let values = line.split('"').collect::<Vec<_>>();
        if values.len() < 2 {
            continue;
        }
        let name = values[1].to_string();
        if name == "name" {
            continue;
        }
        let constraint = values.get(3).copied().unwrap_or("").to_string();
        result.push((name, constraint, category.into()));
    }
    result
}

fn require_relatives(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|raw| {
            let line = raw.trim();
            let rest = line.strip_prefix("require_relative ")?;
            let target = rest.trim().trim_matches(['"', '\'']);
            (!target.is_empty()).then_some(target.to_string())
        })
        .collect()
}

fn resolve_local(source_path: &str, target: &str) -> Option<String> {
    let parent = Path::new(source_path).parent().unwrap_or(Path::new(""));
    let mut candidate = parent.join(target);
    if candidate.extension().is_none() {
        candidate.set_extension("rb");
    }
    Some(candidate.to_string_lossy().replace('\\', "/"))
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !EXCLUDED.contains(&name.as_str()) {
                collect(root, &path, out);
            }
        } else if path.file_name().is_some_and(|n| n == "Gemfile")
            || path
                .extension()
                .is_some_and(|e| e == "gemspec" || e.eq_ignore_ascii_case("rb"))
        {
            out.push(path);
        }
    }
    let _ = root;
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

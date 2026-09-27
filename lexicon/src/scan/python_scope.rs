use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn changed_context(
    source_root: &Path,
    changed_files: &[String],
) -> Result<Vec<String>, std::io::Error> {
    let mut selected = BTreeSet::new();
    for path in changed_files {
        let path = path.replace('\\', "/");
        if path.is_empty() || !source_root.join(path_from_slash(&path)).is_file() {
            continue;
        }
        selected.insert(path.clone());
        add_package_initializers(source_root, &path, &mut selected);
        for module in imported_modules(&source_root.join(path_from_slash(&path)), &path)? {
            for candidate in module_files(source_root, &module) {
                selected.insert(candidate.clone());
                add_package_initializers(source_root, &candidate, &mut selected);
            }
        }
    }
    Ok(selected.into_iter().collect())
}

fn imported_modules(path: &Path, relative: &str) -> Result<Vec<String>, std::io::Error> {
    let text = fs::read_to_string(path)?;
    let mut modules = BTreeSet::new();
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if let Some(rest) = line.strip_prefix("from ") {
            if let Some((module, names)) = rest.split_once(" import ") {
                let base = resolve_relative_module(relative, module.trim());
                if !base.is_empty() {
                    modules.insert(base.clone());
                    for item in names.split(',') {
                        let name = item.split_whitespace().next().unwrap_or("");
                        if !name.is_empty() && name != "*" {
                            modules.insert(format!("{base}.{name}"));
                        }
                    }
                }
            }
        } else if let Some(rest) = line.strip_prefix("import ") {
            for item in rest.split(',') {
                let name = item.split_whitespace().next().unwrap_or("");
                if !name.is_empty() {
                    modules.insert(name.to_owned());
                }
            }
        }
    }
    Ok(modules.into_iter().collect())
}

fn resolve_relative_module(relative: &str, module: &str) -> String {
    if !module.starts_with('.') {
        return module.to_owned();
    }
    let dots = module.bytes().take_while(|byte| *byte == b'.').count();
    let mut parts = Path::new(relative)
        .parent()
        .map(|path| {
            path.components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for _ in 1..dots {
        parts.pop();
    }
    let suffix = &module[dots..];
    if !suffix.is_empty() {
        parts.extend(suffix.split('.').map(str::to_owned));
    }
    parts.join(".")
}

fn module_files(source_root: &Path, module: &str) -> Vec<String> {
    if module.is_empty() {
        return Vec::new();
    }
    let relative = module.replace('.', "/");
    let candidates = [format!("{relative}.py"), format!("{relative}/__init__.py")];
    candidates
        .into_iter()
        .filter(|candidate| source_root.join(path_from_slash(candidate)).is_file())
        .collect()
}

fn add_package_initializers(source_root: &Path, path: &str, selected: &mut BTreeSet<String>) {
    let mut directory = Path::new(path)
        .parent()
        .unwrap_or(Path::new(""))
        .to_path_buf();
    while !directory.as_os_str().is_empty() {
        let candidate = directory.join("__init__.py");
        if source_root.join(&candidate).is_file() {
            selected.insert(candidate.to_string_lossy().replace('\\', "/"));
        }
        if !directory.pop() {
            break;
        }
    }
}

fn path_from_slash(value: &str) -> PathBuf {
    value.split('/').collect()
}

#[cfg(test)]
mod tests {
    use super::resolve_relative_module;

    #[test]
    fn resolves_relative_import_modules() {
        assert_eq!(resolve_relative_module("pkg/sub/a.py", ".b"), "pkg.sub.b");
        assert_eq!(resolve_relative_module("pkg/sub/a.py", "..b"), "pkg.b");
    }
}

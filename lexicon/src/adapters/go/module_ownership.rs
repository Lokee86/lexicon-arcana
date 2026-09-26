use std::path::Path;

use crate::AdapterError;

use super::discovery::{Module, SourceFile};

pub(crate) fn discover(files: &[SourceFile]) -> Result<Vec<Module>, AdapterError> {
    let mut modules = Vec::new();
    for file in files {
        if Path::new(&file.path)
            .file_name()
            .is_none_or(|name| name != "go.mod")
        {
            continue;
        }
        let root = parent(&file.path).unwrap_or(".");
        let content = file
            .manifest_content
            .as_deref()
            .ok_or_else(|| AdapterError::new(format!("missing retained manifest {}", file.path)))?;
        modules.push(Module {
            root: root.to_owned(),
            path: module_path(content)?,
        });
    }
    modules.sort_by(|left, right| {
        left.root
            .cmp(&right.root)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(modules)
}

pub(crate) fn assign(files: &mut [SourceFile], modules: &[Module]) {
    for file in files {
        file.module = index(modules, &file.path);
    }
}

pub(crate) fn index(modules: &[Module], path: &str) -> Option<usize> {
    modules
        .iter()
        .enumerate()
        .filter(|(_, module)| owns(&module.root, path))
        .max_by_key(|(_, module)| module.root.len())
        .map(|(index, _)| index)
}

pub(crate) fn repository_identity(root: &Path, modules: &[Module]) -> String {
    modules
        .iter()
        .find(|module| module.root == ".")
        .map(|module| module.path.clone())
        .unwrap_or_else(|| {
            root.file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("repository")
                .to_owned()
        })
}

fn module_path(content: &[u8]) -> Result<String, AdapterError> {
    for line in String::from_utf8_lossy(content).lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.len() >= 2 && fields[0] == "module" {
            return Ok(fields[1].to_owned());
        }
    }
    Err(AdapterError::new("go.mod has no module directive"))
}

fn owns(root: &str, path: &str) -> bool {
    root == "." || path == root || path.starts_with(&format!("{root}/"))
}

fn parent(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(parent, _)| parent)
}

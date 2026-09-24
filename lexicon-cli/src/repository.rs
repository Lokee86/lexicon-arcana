use std::path::{Path, PathBuf};

pub fn resolve_repository(explicit: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path) = explicit.filter(|value| !value.is_empty()) {
        return absolute_directory(Path::new(path));
    }
    let current = std::env::current_dir().map_err(|error| error.to_string())?;
    discover_repository(&current)
}

pub fn init_repository(explicit: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path) = explicit.filter(|value| !value.is_empty()) {
        return absolute_directory(Path::new(path));
    }
    let current = std::env::current_dir().map_err(|error| error.to_string())?;
    match discover_repository(&current) {
        Ok(root) => Ok(root),
        Err(_) => absolute_directory(&current),
    }
}

fn discover_repository(start: &Path) -> Result<PathBuf, String> {
    let mut root = absolute_directory(start)?;
    loop {
        let configuration = lexicon::config_path(&root);
        match std::fs::metadata(&configuration) {
            Ok(_) => return Ok(root),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("find Lexicon configuration: {error}")),
        }
        let Some(parent) = root.parent() else {
            break;
        };
        if parent == root {
            break;
        }
        root = parent.to_path_buf();
    }
    Err(format!(
        "Lexicon repository not found from {}; use --repo or run lexicon init",
        start.display()
    ))
}

fn absolute_directory(path: &Path) -> Result<PathBuf, String> {
    let value = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    let value = value
        .canonicalize()
        .map_err(|error| format!("resolve repository {}: {error}", path.display()))?;
    if !value.is_dir() {
        return Err(format!(
            "repository is not a directory: {}",
            value.display()
        ));
    }
    Ok(value)
}

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::RepositoryError;

pub fn prepare_state_directory(
    repository_root: &Path,
    state_dir: &Path,
) -> Result<(), RepositoryError> {
    let state = absolute(state_dir)?;
    fs::create_dir_all(&state)
        .map_err(|error| RepositoryError::new(format!("create state directory: {error}")))?;
    mark_hidden(&state)?;

    if repository_root.as_os_str().is_empty() {
        return Ok(());
    }
    let root = absolute(repository_root)?;
    let Ok(relative) = state.strip_prefix(&root) else {
        return Ok(());
    };
    if relative.as_os_str().is_empty() {
        return Ok(());
    }

    let first = relative
        .components()
        .next()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_default();
    if first.starts_with('.') {
        let top = root.join(&first);
        if top != state {
            mark_hidden(&top)?;
        }
    }
    ensure_ignored(&root, relative)
}

fn ensure_ignored(root: &Path, relative: &Path) -> Result<(), RepositoryError> {
    let entry = ignore_entry(relative);
    let path = root.join(".gitignore");
    let data = match fs::read(&path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return Err(RepositoryError::new(format!("read .gitignore: {error}")));
        }
    };
    let text = String::from_utf8_lossy(&data).replace("\r\n", "\n");
    if text
        .lines()
        .any(|line| equivalent_ignore(line.trim(), &entry))
    {
        return Ok(());
    }

    let newline = if data.windows(2).any(|window| window == b"\r\n") {
        b"\r\n".as_slice()
    } else {
        b"\n".as_slice()
    };
    let mut updated = data;
    if !updated.is_empty() && !updated.ends_with(b"\n") {
        updated.extend_from_slice(newline);
    }
    updated.extend_from_slice(entry.as_bytes());
    updated.extend_from_slice(newline);
    fs::write(&path, updated)
        .map_err(|error| RepositoryError::new(format!("update .gitignore: {error}")))
}

fn ignore_entry(relative: &Path) -> String {
    let value = relative.to_string_lossy().replace('\\', "/");
    let value = value.trim_matches('/');
    let first = value.split('/').next().unwrap_or_default();
    let value = if value.contains('/') && first.starts_with('.') {
        first
    } else {
        value
    };
    format!("/{value}/")
}

fn equivalent_ignore(line: &str, entry: &str) -> bool {
    if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
        return false;
    }
    let normalize = |value: &str| {
        value
            .trim()
            .trim_start_matches('/')
            .trim_end_matches('/')
            .to_owned()
    };
    normalize(line) == normalize(entry)
}

fn absolute(path: &Path) -> Result<PathBuf, RepositoryError> {
    if path.is_absolute() {
        return Ok(crate::config::clean_path(path));
    }
    std::env::current_dir()
        .map(|current| crate::config::clean_path(&current.join(path)))
        .map_err(|error| RepositoryError::new(format!("resolve state directory: {error}")))
}

#[cfg(windows)]
fn mark_hidden(path: &Path) -> Result<(), RepositoryError> {
    let status = Command::new("attrib")
        .arg("+H")
        .arg(path)
        .status()
        .map_err(|error| RepositoryError::new(format!("hide state directory: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(RepositoryError::new(format!(
            "hide state directory: attrib exited with {status}"
        )))
    }
}

#[cfg(not(windows))]
fn mark_hidden(_path: &Path) -> Result<(), RepositoryError> {
    Ok(())
}

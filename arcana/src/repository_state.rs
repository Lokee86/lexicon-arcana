use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn prepare(state: &Path, lexicon_root: &Path) -> io::Result<()> {
    let state = absolute(state)?;
    fs::create_dir_all(&state)?;
    mark_hidden(&state)?;

    let repository = find_repository_root(&state).or_else(|| {
        absolute(lexicon_root)
            .ok()
            .and_then(|path| find_repository_root(&path))
    });
    let Some(repository) = repository else {
        return Ok(());
    };
    let Ok(relative) = state.strip_prefix(&repository) else {
        return Ok(());
    };
    if let Some(first) = relative.components().next()
        && first.as_os_str().to_string_lossy().starts_with('.')
    {
        let top = repository.join(first.as_os_str());
        if top != state {
            mark_hidden(&top)?;
        }
    }
    ensure_ignored(&repository, relative)
}

fn find_repository_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|path| path.join(".git").exists())
        .map(Path::to_owned)
}

fn ensure_ignored(repository: &Path, relative: &Path) -> io::Result<()> {
    let mut value = relative.to_string_lossy().replace('\\', "/");
    value = value.trim_matches('/').to_owned();
    if let Some((first, _)) = value.split_once('/')
        && first.starts_with('.')
    {
        value = first.to_owned();
    }
    let entry = format!("/{value}/");
    let path = repository.join(".gitignore");
    let data = match fs::read(&path) {
        Ok(data) => data,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    let normalized = String::from_utf8_lossy(&data).replace("\r\n", "\n");
    if normalized
        .lines()
        .any(|line| equivalent_ignore(line, &entry))
    {
        return Ok(());
    }

    let newline = if data.windows(2).any(|pair| pair == b"\r\n") {
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
    fs::write(path, updated)
}

fn equivalent_ignore(line: &str, entry: &str) -> bool {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
        return false;
    }
    let line = line.trim_start_matches('/').trim_end_matches('/');
    let entry = entry.trim().trim_start_matches('/').trim_end_matches('/');
    line == entry
}

fn absolute(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

#[cfg(windows)]
fn mark_hidden(path: &Path) -> io::Result<()> {
    let output = std::process::Command::new("attrib")
        .arg("+H")
        .arg(path)
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "attrib +H failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )))
}

#[cfg(not(windows))]
fn mark_hidden(_: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn prepare_ignores_managed_state_once() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("arcana-state-{}-{nonce}", std::process::id()));
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".gitignore"), b"target/\r\n").unwrap();
        let state = root.join(".warlock/tools/arcana");
        let lexicon = root.join(".warlock/tools/lexicon");

        prepare(&state, &lexicon).unwrap();
        prepare(&state, &lexicon).unwrap();

        let ignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        assert_eq!(ignore.matches("/.warlock/").count(), 1);
        assert!(ignore.contains("\r\n/.warlock/\r\n"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn prepare_keeps_nested_ordinary_parent_scoped() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("arcana-nested-{}-{nonce}", std::process::id()));
        fs::create_dir_all(root.join(".git")).unwrap();
        let state = root.join("src/.arcana");
        prepare(&state, &root.join(".lexicon")).unwrap();
        let ignore = fs::read_to_string(root.join(".gitignore")).unwrap();
        assert_eq!(ignore, "/src/.arcana/\n");
        fs::remove_dir_all(root).unwrap();
    }
}

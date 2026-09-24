use std::path::{Path, PathBuf};

pub fn find_adapter_root(repository: &Path, explicit: Option<&Path>) -> Result<PathBuf, String> {
    let executable = std::env::current_exe().ok();
    let current = std::env::current_dir().ok();
    find_adapter_root_from(
        repository,
        explicit,
        std::env::var_os("LEXICON_ADAPTERS")
            .map(PathBuf::from)
            .as_deref(),
        executable.as_deref(),
        current.as_deref(),
    )
}

pub fn find_adapter_root_from(
    repository: &Path,
    explicit: Option<&Path>,
    environment: Option<&Path>,
    executable: Option<&Path>,
    current: Option<&Path>,
) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    push_candidate(&mut candidates, explicit);
    push_candidate(&mut candidates, environment);
    if let Some(executable) = executable.and_then(Path::parent) {
        candidates.push(executable.join("adapters"));
        candidates.push(executable.join("..").join("adapters"));
    }
    candidates.push(repository.join("adapters"));
    if let Some(current) = current {
        candidates.push(current.join("adapters"));
    }

    for candidate in candidates {
        let absolute = absolute(&candidate)?;
        if absolute.join("python").is_dir() {
            return Ok(absolute);
        }
    }
    Err("adapter root not found; use --adapters or LEXICON_ADAPTERS".into())
}

fn push_candidate(candidates: &mut Vec<PathBuf>, value: Option<&Path>) {
    if let Some(value) = value
        && !value.as_os_str().is_empty()
    {
        candidates.push(value.to_path_buf());
    }
}

fn absolute(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(super::path::clean_path(path));
    }
    std::env::current_dir()
        .map(|current| super::path::clean_path(&current.join(path)))
        .map_err(|error| error.to_string())
}

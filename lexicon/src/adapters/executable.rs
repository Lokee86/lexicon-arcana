use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::AdapterError;

pub(crate) fn packaged_executable(root: &Path, language: &str) -> Option<PathBuf> {
    let base = root.join(language).join(format!("lexicon-{language}"));
    let mut candidates = vec![base.clone()];
    if cfg!(windows) {
        candidates.push(base.with_extension("exe"));
    }
    candidates.into_iter().find(|candidate| candidate.is_file())
}

pub(crate) fn find_executable(names: &[&str]) -> Result<PathBuf, AdapterError> {
    for name in names {
        if let Some(path) = find_on_path(name) {
            return Ok(path);
        }
    }
    if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let home = PathBuf::from(home);
        for name in names {
            let executable = executable_name(name);
            for candidate in [
                home.join(".cargo").join("bin").join(&executable),
                home.join("go").join("bin").join(&executable),
            ] {
                if candidate.is_file() {
                    return Ok(candidate);
                }
            }
        }
    }
    Err(AdapterError::new(format!(
        "required executable not found: {}",
        names.join(" or ")
    )))
}

pub(crate) fn npm_executable() -> &'static str {
    if cfg!(windows) { "npm.cmd" } else { "npm" }
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&paths) {
        let plain = directory.join(name);
        if plain.is_file() {
            return Some(plain);
        }
        if cfg!(windows) {
            for extension in ["exe", "cmd", "bat"] {
                let candidate = directory.join(format!("{name}.{extension}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn executable_name(name: &str) -> OsString {
    if cfg!(windows) && Path::new(name).extension().is_none() {
        OsString::from(format!("{name}.exe"))
    } else {
        OsString::from(name)
    }
}

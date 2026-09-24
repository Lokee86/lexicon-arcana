use std::path::{Path, PathBuf};

pub(super) fn check_command(command: &str) -> Result<(), String> {
    find_executable(command)
        .map(|_| ())
        .ok_or_else(|| format!("required executable not found: {command}"))
}

fn find_executable(command: &str) -> Option<PathBuf> {
    let path = Path::new(command);
    if path.components().count() > 1 {
        return executable_file(path).then(|| path.to_path_buf());
    }
    let search = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&search) {
        for candidate in executable_candidates(&directory, command) {
            if executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(windows)]
fn executable_candidates(directory: &Path, command: &str) -> Vec<PathBuf> {
    let direct = directory.join(command);
    if Path::new(command).extension().is_some() {
        return vec![direct];
    }
    let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    extensions
        .split(';')
        .filter(|value| !value.is_empty())
        .map(|extension| directory.join(format!("{command}{extension}")))
        .chain(std::iter::once(direct))
        .collect()
}

#[cfg(not(windows))]
fn executable_candidates(directory: &Path, command: &str) -> Vec<PathBuf> {
    vec![directory.join(command)]
}

#[cfg(unix)]
fn executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn executable_file(path: &Path) -> bool {
    path.is_file()
}

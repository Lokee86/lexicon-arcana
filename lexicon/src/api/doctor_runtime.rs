use std::path::{Path, PathBuf};

const RUNTIME_REQUIREMENTS: &[(&str, &[&[&str]])] = &[
    ("c-family", &[&["go"]]),
    ("go", &[&["go"]]),
    ("gdscript", &[&["go"]]),
    ("generic", &[&["go"]]),
    ("java", &[&["go"]]),
    ("kotlin", &[&["go"]]),
    ("lotusscript", &[&["go"]]),
    ("csharp", &[&["dotnet"]]),
    ("python", &[&["python", "python3"]]),
    ("ruby", &[&["ruby"]]),
    ("rust", &[&["cargo"]]),
    ("typescript", &[&["node"], &["npm", "npm.cmd"]]),
];

pub(super) fn check_runtime(adapter_root: &Path, language: &str) -> Result<(), String> {
    if packaged_runtime_available(adapter_root, language) {
        return Ok(());
    }
    let Some((_, configured)) = RUNTIME_REQUIREMENTS
        .iter()
        .find(|(candidate, _)| *candidate == language)
    else {
        return Err(format!(
            "no runtime definition for detected language {language:?}"
        ));
    };

    let node_only = language == "typescript"
        && adapter_root
            .join("typescript")
            .join("dist")
            .join("cli.js")
            .is_file();
    let requirements: &[&[&str]] = if node_only { &[&["node"]] } else { configured };
    for candidates in requirements {
        if !candidates
            .iter()
            .any(|candidate| find_executable(candidate).is_some())
        {
            return Err(format!(
                "required executable not found: {}",
                candidates.join(" or ")
            ));
        }
    }
    Ok(())
}

pub(super) fn check_command(command: &str) -> Result<(), String> {
    find_executable(command)
        .map(|_| ())
        .ok_or_else(|| format!("required executable not found: {command}"))
}

fn packaged_runtime_available(adapter_root: &Path, language: &str) -> bool {
    let base = adapter_root
        .join(language)
        .join(format!("lexicon-{language}"));
    base.is_file() || base.with_extension("exe").is_file()
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

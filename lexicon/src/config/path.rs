use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

pub fn state_root(repository: &Path) -> PathBuf {
    state_root_from(repository, std::env::var_os("LEXICON_STATE_DIR").as_deref())
}

pub fn config_path(repository: &Path) -> PathBuf {
    state_root(repository).join("config.json")
}

pub(crate) fn state_root_from(repository: &Path, configured: Option<&OsStr>) -> PathBuf {
    match configured {
        Some(configured) => {
            let configured = PathBuf::from(configured);
            if configured.is_absolute() {
                clean_path(&configured)
            } else {
                clean_path(&repository.join(configured))
            }
        }
        None => repository.join(".lexicon"),
    }
}

pub(crate) fn clean_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !result.pop() {
                    result.push(component.as_os_str());
                }
            }
            _ => result.push(component.as_os_str()),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_root_matches_go_override_rules() {
        let repository = Path::new("C:/repo");
        assert_eq!(
            state_root_from(repository, None),
            repository.join(".lexicon")
        );
        assert_eq!(
            state_root_from(repository, Some(OsStr::new(".warlock/tools/lexicon"))),
            repository.join(".warlock/tools/lexicon")
        );
        let absolute = if cfg!(windows) {
            PathBuf::from("D:/lexicon-state")
        } else {
            PathBuf::from("/tmp/lexicon-state")
        };
        assert_eq!(
            state_root_from(repository, Some(absolute.as_os_str())),
            absolute
        );
    }
}

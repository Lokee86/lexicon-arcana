use std::fs;
use std::path::{Path, PathBuf};

use crate::{LexiconError, StorageError, Store, load_config, state_root};

use super::Lexicon;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusReport {
    pub repository_root: PathBuf,
    pub current_snapshot_id: Option<String>,
    pub detected_languages: Vec<String>,
    pub enabled_languages: Vec<String>,
    pub registered_consumers: Vec<String>,
}

pub fn status(repository: impl AsRef<Path>) -> Result<StatusReport, LexiconError> {
    let repository = absolute(repository.as_ref())?;
    let config = load_config(&repository).map_err(LexiconError::new)?;
    let state_root = state_root(&repository);
    let store = Store::new(&state_root);
    let (current_snapshot_id, mut detected_languages) = match store.current() {
        Ok((id, manifest)) => {
            let mut languages = manifest
                .languages
                .unwrap_or_default()
                .into_iter()
                .map(|entry| entry.language)
                .collect::<Vec<_>>();
            languages.sort();
            (Some(id), languages)
        }
        Err(StorageError::NoCurrentSnapshot) => (None, Vec::new()),
        Err(error) => return Err(error.into()),
    };
    detected_languages.dedup();

    Ok(StatusReport {
        repository_root: repository,
        current_snapshot_id,
        detected_languages,
        enabled_languages: config.enabled_languages,
        registered_consumers: registered_consumer_names(&state_root)?,
    })
}

impl Lexicon {
    pub fn status(&self) -> Result<StatusReport, LexiconError> {
        status(self.repository())
    }
}

fn registered_consumer_names(state_root: &Path) -> Result<Vec<String>, LexiconError> {
    let entries = match fs::read_dir(state_root.join("consumers")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(LexiconError::new(format!(
                "read Lexicon consumers: {error}"
            )));
        }
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        if let Some(stem) = entry.path().file_stem().and_then(|value| value.to_str()) {
            names.push(stem.to_owned());
        }
    }
    names.sort();
    Ok(names)
}

fn absolute(path: &Path) -> Result<PathBuf, LexiconError> {
    if path.is_absolute() {
        return Ok(crate::config::clean_path(path));
    }
    std::env::current_dir()
        .map(|current| crate::config::clean_path(&current.join(path)))
        .map_err(Into::into)
}

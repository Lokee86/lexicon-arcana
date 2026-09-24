use std::fs;

use crate::{LexiconError, StorageError, load_config};

use super::Lexicon;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusReport {
    pub repository_root: std::path::PathBuf,
    pub current_snapshot_id: Option<String>,
    pub detected_languages: Vec<String>,
    pub enabled_languages: Vec<String>,
    pub registered_consumers: Vec<String>,
}

impl Lexicon {
    pub fn status(&self) -> Result<StatusReport, LexiconError> {
        let config = load_config(self.repository()).map_err(LexiconError::new)?;
        let (current_snapshot_id, mut detected_languages) = match self.store().current() {
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
            repository_root: self.repository().to_path_buf(),
            current_snapshot_id,
            detected_languages,
            enabled_languages: config.enabled_languages,
            registered_consumers: registered_consumer_names(self.state_root())?,
        })
    }
}

fn registered_consumer_names(state_root: &std::path::Path) -> Result<Vec<String>, LexiconError> {
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

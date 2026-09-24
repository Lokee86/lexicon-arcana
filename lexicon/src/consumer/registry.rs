use std::fs;
use std::path::{Path, PathBuf};

use crate::LexiconError;

use super::ConsumerDefinition;

pub fn list_consumer_paths(root: &Path) -> Result<Vec<PathBuf>, LexiconError> {
    let entries = match fs::read_dir(root.join("consumers")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(LexiconError::new(format!(
                "read Lexicon consumers: {error}"
            )));
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        paths.push(entry.path());
    }
    paths.sort();
    Ok(paths)
}

pub fn load_consumer_definition(path: &Path) -> Result<ConsumerDefinition, LexiconError> {
    let data = fs::read(path).map_err(|error| {
        LexiconError::new(format!("read Lexicon consumer {}: {error}", path.display()))
    })?;
    ConsumerDefinition::parse(&data)
        .map_err(|error| LexiconError::new(format!("Lexicon consumer {}: {error}", path.display())))
}

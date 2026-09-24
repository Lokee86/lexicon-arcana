use std::path::Path;

use super::io::write_atomic;
use super::{LanguageEntry, SnapshotManifest, StorageError, Store};

impl Store {
    pub fn export(
        &self,
        snapshot: &str,
        destination: &Path,
        languages: &[String],
    ) -> Result<(), StorageError> {
        let manifest = self.resolve_export_snapshot(snapshot)?;
        let selected = select_languages(&manifest, languages)?;
        let mut outputs = Vec::with_capacity(selected.len());
        for entry in selected {
            let data = self.export_language(&entry)?;
            outputs.push((entry.language, data));
        }
        for (language, data) in outputs {
            let path = destination.join(format!("{language}.jsonl"));
            write_atomic(&path, &data)
                .map_err(|error| operation(format!("publish {language} library: {error}")))?;
        }
        Ok(())
    }

    fn resolve_export_snapshot(&self, snapshot: &str) -> Result<SnapshotManifest, StorageError> {
        if snapshot.is_empty() || snapshot.eq_ignore_ascii_case("CURRENT") {
            return self.current().map(|(_, manifest)| manifest);
        }
        self.load_snapshot(snapshot)
    }
}

pub(crate) fn select_languages(
    manifest: &SnapshotManifest,
    requested: &[String],
) -> Result<Vec<LanguageEntry>, StorageError> {
    let mut entries = std::collections::BTreeMap::new();
    for entry in manifest.languages.as_deref().unwrap_or_default() {
        if !valid_language(&entry.language) {
            return Err(operation(format!(
                "invalid snapshot language {:?}",
                entry.language
            )));
        }
        if entries
            .insert(entry.language.clone(), entry.clone())
            .is_some()
        {
            return Err(operation(format!(
                "snapshot contains duplicate language {:?}",
                entry.language
            )));
        }
    }

    let names = if requested.is_empty() {
        entries.keys().cloned().collect::<Vec<_>>()
    } else {
        let mut values = requested.to_vec();
        values.sort();
        values.dedup();
        values
    };
    names
        .into_iter()
        .map(|language| {
            entries
                .get(&language)
                .cloned()
                .ok_or_else(|| operation(format!("snapshot has no {language} library")))
        })
        .collect()
}

pub(crate) fn valid_language(language: &str) -> bool {
    !language.is_empty()
        && language != "."
        && language != ".."
        && !language.contains('/')
        && !language.contains('\\')
}

pub(crate) fn valid_path(path: &str) -> bool {
    if path.is_empty() || path == "." || path.contains('\\') {
        return false;
    }
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return false;
    }
    let mut parts = Vec::new();
    for component in candidate.components() {
        match component {
            std::path::Component::Normal(value) => parts.push(value),
            _ => return false,
        }
    }
    !parts.is_empty()
        && parts
            .iter()
            .map(|part| part.to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
            == path
}

pub(crate) fn operation(message: impl Into<String>) -> StorageError {
    StorageError::Operation(message.into())
}

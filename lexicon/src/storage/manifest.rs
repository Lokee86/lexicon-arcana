use super::{LanguageEntry, SnapshotManifest};

impl SnapshotManifest {
    pub fn language(&self, language: &str) -> Option<&LanguageEntry> {
        self.languages
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|entry| entry.language == language)
    }

    pub fn with_language(mut self, entry: LanguageEntry) -> Self {
        let mut languages = self.languages.take().unwrap_or_default();
        if let Some(current) = languages
            .iter_mut()
            .find(|current| current.language == entry.language)
        {
            *current = entry;
        } else {
            languages.push(entry);
        }
        languages.sort_by(|left, right| left.language.cmp(&right.language));
        self.languages = Some(languages);
        self
    }

    pub fn without_language(mut self, language: &str) -> Self {
        let mut languages = self.languages.take().unwrap_or_default();
        languages.retain(|entry| entry.language != language);
        self.languages = Some(languages);
        self
    }
}

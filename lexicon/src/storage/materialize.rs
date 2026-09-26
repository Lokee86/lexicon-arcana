use std::collections::{BTreeMap, BTreeSet};

use super::analysis::normalized_paths;
use super::materialize_parallel::write_full_file_objects;
use super::materialize_support::{
    language_metadata, materialization, require_incremental_scope, require_language, source_map,
};
use super::{Analysis, FileEntry, LanguageEntry, SourceFile, StorageError, Store};

impl Store {
    pub fn build_full_language(
        &self,
        analysis: &Analysis,
        sources: &[SourceFile],
        language: &str,
        analysis_config_id: &str,
        adapter_fingerprint: &str,
    ) -> Result<LanguageEntry, StorageError> {
        require_language(analysis, language)?;
        if analysis.is_incremental() {
            return Err(materialization(
                "full materialization received incremental analysis",
            ));
        }
        let sources = source_map(sources)?;
        let allowed: BTreeSet<String> = sources.keys().cloned().collect();
        let groups = analysis.groups(Some(&allowed));
        let entry = language_metadata(analysis, analysis_config_id, adapter_fingerprint);

        let files = write_full_file_objects(self, &entry, sources, &groups)?;
        let shared_object_id = self.write_language_shared_object(&entry, groups.shared)?;
        Ok(LanguageEntry {
            files: Some(files),
            shared_object_id,
            ..entry
        })
    }

    pub fn build_shared_language(
        &self,
        analysis: &Analysis,
        analysis_config_id: &str,
        adapter_fingerprint: &str,
    ) -> Result<LanguageEntry, StorageError> {
        if analysis.is_incremental() {
            return Err(materialization(
                "shared materialization received incremental analysis",
            ));
        }
        let entry = language_metadata(analysis, analysis_config_id, adapter_fingerprint);
        let shared_object_id =
            self.write_language_shared_object(&entry, analysis.records.clone())?;
        Ok(LanguageEntry {
            files: Some(Vec::new()),
            shared_object_id,
            ..entry
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_incremental_language(
        &self,
        previous: &LanguageEntry,
        analysis: &Analysis,
        changed_sources: &[SourceFile],
        analysis_config_id: &str,
        adapter_fingerprint: &str,
        changed_files: &[String],
        removed_files: &[String],
        replace_shared: bool,
    ) -> Result<LanguageEntry, StorageError> {
        if !analysis.is_incremental() {
            return Err(materialization(
                "incremental materialization requires incremental analysis",
            ));
        }
        require_language(analysis, &previous.language)?;
        require_incremental_scope(analysis, changed_files, removed_files)?;

        let changed: BTreeSet<String> = normalized_paths(changed_files).into_iter().collect();
        let removed: BTreeSet<String> = normalized_paths(removed_files).into_iter().collect();
        let groups = analysis.groups(None);
        validate_incremental_owners(groups.owned.keys(), &changed, &removed)?;

        let sources = source_map(changed_sources)?;
        let entry = language_metadata(analysis, analysis_config_id, adapter_fingerprint);
        let mut files = retained_files(previous, &changed, &removed);

        for path in changed.iter().filter(|path| !removed.contains(*path)) {
            let source = sources
                .get(path)
                .ok_or_else(|| materialization(format!("missing changed source {path:?}")))?;
            let file = self.write_language_file_object(
                &entry,
                path,
                source,
                groups.owned.get(path).cloned().unwrap_or_default(),
            )?;
            files.insert(path.clone(), file);
        }

        let shared_object_id = if replace_shared {
            self.write_language_shared_object(&entry, groups.shared)?
        } else {
            previous.shared_object_id.clone()
        };
        Ok(LanguageEntry {
            files: Some(files.into_values().collect()),
            shared_object_id,
            ..entry
        })
    }
}

fn retained_files(
    previous: &LanguageEntry,
    changed: &BTreeSet<String>,
    removed: &BTreeSet<String>,
) -> BTreeMap<String, FileEntry> {
    previous
        .files
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|file| !changed.contains(&file.path) && !removed.contains(&file.path))
        .map(|file| (file.path.clone(), file.clone()))
        .collect()
}

fn validate_incremental_owners<'a>(
    owners: impl Iterator<Item = &'a String>,
    changed: &BTreeSet<String>,
    removed: &BTreeSet<String>,
) -> Result<(), StorageError> {
    for owner in owners {
        if !changed.contains(owner) {
            return Err(materialization(format!(
                "incremental record is owned by undeclared file {owner:?}"
            )));
        }
        if removed.contains(owner) {
            return Err(materialization(format!(
                "incremental record is owned by removed file {owner:?}"
            )));
        }
    }
    Ok(())
}

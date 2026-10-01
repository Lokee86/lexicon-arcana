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
        let source_bytes = sources
            .values()
            .map(|source| source.len() as u64)
            .sum::<u64>();
        let allowed: BTreeSet<String> = sources.keys().map(|path| (*path).to_owned()).collect();
        let groups = analysis.groups(Some(&allowed));
        let owned_records = groups
            .owned
            .values()
            .map(|records| records.len() as u64)
            .sum::<u64>();
        let shared_records = groups.shared.len() as u64;
        let entry = language_metadata(analysis, analysis_config_id, adapter_fingerprint);

        let objects_started = crate::perf::start();
        let files = write_full_file_objects(self, &entry, sources, &groups)?;
        let shared_object_id = self.write_language_shared_object(&entry, &groups.shared)?;
        if let Some(objects_started) = objects_started {
            crate::perf::emit(
                &format!("{}.object_construction_cas", language),
                objects_started.elapsed(),
                &[
                    ("file_objects", files.len() as u64),
                    ("shared_objects", u64::from(!shared_object_id.is_empty())),
                    ("owned_records", owned_records),
                    ("shared_records", shared_records),
                    ("record_clones", 0),
                    ("record_references", owned_records + shared_records),
                    ("source_bytes", source_bytes),
                ],
            );
        }
        let mut materialized = LanguageEntry {
            files: Some(files),
            shared_object_id,
            ..entry
        };
        // Full analysis already owns all canonical records. Build the index
        // directly from those borrowed records, without decoding fact objects.
        materialized.dependency_index_id = self.index_full_language(&materialized, &groups)?;
        Ok(materialized)
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
        let shared_object_id = self.write_language_shared_records(&entry, &analysis.records)?;
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
                .get(path.as_str())
                .ok_or_else(|| materialization(format!("missing changed source {path:?}")))?;
            let records = groups
                .owned
                .get(path)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let file = self.write_language_file_object(&entry, path, source, records)?;
            files.insert(path.clone(), file);
        }

        let shared_object_id = if replace_shared {
            self.merge_language_shared_object(
                &entry,
                previous,
                &groups.shared,
                &groups.owned,
                changed_files,
                removed_files,
            )?
        } else {
            previous.shared_object_id.clone()
        };
        if shared_object_id != previous.shared_object_id {
            return Err(StorageError::UnsafeIndexDelta(
                "shared fact replacement requires full analysis".into(),
            ));
        }
        let mut materialized = LanguageEntry {
            files: Some(files.into_values().collect()),
            shared_object_id,
            ..entry
        };
        materialized.dependency_index_id = self.index_incremental_language(
            previous,
            &materialized,
            &groups.owned,
            &changed,
            &removed,
        )?;
        Ok(materialized)
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

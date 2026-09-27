use std::collections::BTreeMap;

use crate::{FactRecord, content_id};

use super::analysis::normalized_paths;
use super::binary::{ObjectView, RecordSelection};
use super::{Analysis, FileEntry, LanguageEntry, SourceFile, StorageError, Store};

pub(crate) fn source_map<'a>(
    sources: &'a [SourceFile],
) -> Result<BTreeMap<&'a str, &'a [u8]>, StorageError> {
    let started = crate::perf::start();
    let source_bytes = sources
        .iter()
        .map(|source| source.content.len() as u64)
        .sum::<u64>();
    let mut result = BTreeMap::new();
    for source in sources {
        let normalized = normalized_paths(std::slice::from_ref(&source.path));
        if normalized.len() != 1 || normalized[0] != source.path {
            return Err(materialization(format!(
                "source path is not canonical: {:?}",
                source.path
            )));
        }
        if result
            .insert(source.path.as_str(), source.content.as_slice())
            .is_some()
        {
            return Err(materialization(format!(
                "duplicate source path {:?}",
                source.path
            )));
        }
    }
    if let Some(started) = started {
        crate::perf::emit(
            "storage.source_copy",
            started.elapsed(),
            &[
                ("source_files", sources.len() as u64),
                ("source_bytes", source_bytes),
                ("cloned_source_bytes", 0),
            ],
        );
    }
    Ok(result)
}

pub(crate) fn language_metadata(
    analysis: &Analysis,
    analysis_config_id: &str,
    adapter_fingerprint: &str,
) -> LanguageEntry {
    LanguageEntry {
        language: analysis.header.language.clone(),
        adapter_version: analysis.header.adapter_version.clone(),
        adapter_fingerprint: adapter_fingerprint.to_owned(),
        schema_version: analysis.header.schema_version.into(),
        repository: analysis.header.repository.clone(),
        analysis_config_id: analysis_config_id.to_owned(),
        shared_object_id: String::new(),
        files: None,
    }
}

pub(crate) fn require_language(analysis: &Analysis, language: &str) -> Result<(), StorageError> {
    if analysis.header.language == language {
        Ok(())
    } else {
        Err(materialization(format!(
            "analysis language {:?} does not match {language:?}",
            analysis.header.language
        )))
    }
}

pub(crate) fn require_incremental_scope(
    analysis: &Analysis,
    changed: &[String],
    removed: &[String],
) -> Result<(), StorageError> {
    if normalized_paths(analysis.header.changed_files.as_deref().unwrap_or_default())
        != normalized_paths(changed)
        || normalized_paths(analysis.header.removed_files.as_deref().unwrap_or_default())
            != normalized_paths(removed)
    {
        return Err(materialization(
            "adapter incremental scope does not match requested files",
        ));
    }
    Ok(())
}

pub(crate) fn materialization(message: impl Into<String>) -> StorageError {
    StorageError::Materialization(message.into())
}

impl Store {
    pub(crate) fn write_language_file_object(
        &self,
        entry: &LanguageEntry,
        path: &str,
        source: &[u8],
        records: &[&FactRecord],
    ) -> Result<FileEntry, StorageError> {
        let source_content_id = content_id(source);
        let object_id = self.write_object_view(&ObjectView {
            language: &entry.language,
            owner: path,
            source_content_id: &source_content_id,
            adapter_version: &entry.adapter_version,
            schema_version: entry.schema_version,
            analysis_config_id: &entry.analysis_config_id,
            records: RecordSelection::Refs(records),
        })?;
        Ok(FileEntry {
            path: path.to_owned(),
            language: entry.language.clone(),
            content_id: source_content_id,
            object_id,
        })
    }

    pub(crate) fn write_language_shared_object(
        &self,
        entry: &LanguageEntry,
        records: &[&FactRecord],
    ) -> Result<String, StorageError> {
        self.write_language_shared_selection(entry, RecordSelection::Refs(records))
    }

    pub(crate) fn write_language_shared_records(
        &self,
        entry: &LanguageEntry,
        records: &[FactRecord],
    ) -> Result<String, StorageError> {
        self.write_language_shared_selection(entry, RecordSelection::Direct(records))
    }

    fn write_language_shared_selection(
        &self,
        entry: &LanguageEntry,
        records: RecordSelection<'_>,
    ) -> Result<String, StorageError> {
        if records.is_empty() {
            return Ok(String::new());
        }
        self.write_object_view(&ObjectView {
            language: &entry.language,
            owner: "",
            source_content_id: "",
            adapter_version: &entry.adapter_version,
            schema_version: entry.schema_version,
            analysis_config_id: &entry.analysis_config_id,
            records,
        })
    }
}

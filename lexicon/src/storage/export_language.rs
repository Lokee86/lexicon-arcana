use crate::{FACT_SCHEMA_VERSION, FactHeader, FactRecord, FactStream};

use super::export::{operation, valid_path};
use super::{FactObject, LanguageEntry, StorageError, Store};

impl Store {
    pub fn export_language(&self, entry: &LanguageEntry) -> Result<Vec<u8>, StorageError> {
        validate_entry(entry)?;
        let mut records = Vec::<FactRecord>::new();

        if !entry.shared_object_id.is_empty() {
            let object = self.load_object(&entry.shared_object_id).map_err(|error| {
                operation(format!(
                    "load shared object {}: {error}",
                    entry.shared_object_id
                ))
            })?;
            validate_object(&object, entry, None)?;
            records.extend(object.records);
        }

        let mut files = entry.files.clone().unwrap_or_default();
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let mut previous = None::<String>;
        for file in files {
            if !valid_path(&file.path) {
                return Err(operation(format!("invalid file path {:?}", file.path)));
            }
            if previous.as_deref() == Some(file.path.as_str()) {
                return Err(operation(format!("duplicate file path {:?}", file.path)));
            }
            previous = Some(file.path.clone());
            if file.language != entry.language
                || file.content_id.is_empty()
                || file.object_id.is_empty()
            {
                return Err(operation(format!(
                    "invalid metadata for file {:?}",
                    file.path
                )));
            }
            let object = self.load_object(&file.object_id).map_err(|error| {
                operation(format!("load file object {}: {error}", file.object_id))
            })?;
            validate_object(&object, entry, Some((&file.path, &file.content_id)))?;
            records.extend(object.records);
        }

        let schema_version = u32::try_from(entry.schema_version)
            .map_err(|_| operation("snapshot schema version does not fit facts-v1 header"))?;
        let mut stream = FactStream {
            header: FactHeader {
                adapter_version: entry.adapter_version.clone(),
                changed_files: None,
                language: entry.language.clone(),
                mode: Some("full".into()),
                record: "lexicon".into(),
                removed_files: None,
                repository: entry.repository.clone(),
                schema_version,
                shared_complete: None,
            },
            records,
        };
        stream
            .sort_records_for_export()
            .map_err(StorageError::from)?;
        stream.jsonl_unchecked().map_err(StorageError::from)
    }
}

fn validate_entry(entry: &LanguageEntry) -> Result<(), StorageError> {
    if entry.adapter_version.is_empty()
        || entry.schema_version != u64::from(FACT_SCHEMA_VERSION)
        || entry.repository.is_empty()
        || entry.analysis_config_id.is_empty()
    {
        return Err(operation("invalid export language metadata"));
    }
    Ok(())
}

fn validate_object(
    object: &FactObject,
    entry: &LanguageEntry,
    file: Option<(&str, &str)>,
) -> Result<(), StorageError> {
    if object.language != entry.language
        || object.adapter_version != entry.adapter_version
        || object.schema_version != entry.schema_version
        || object.analysis_config_id != entry.analysis_config_id
    {
        return Err(operation(format!(
            "{} object metadata does not match {} manifest",
            if file.is_some() { "file" } else { "shared" },
            entry.language
        )));
    }
    match file {
        None if !object.owner.is_empty() || !object.source_content_id.is_empty() => {
            Err(operation("shared object has file ownership metadata"))
        }
        Some((owner, content_id))
            if object.owner != owner || object.source_content_id != content_id =>
        {
            Err(operation(format!(
                "file object metadata does not match {owner} manifest"
            )))
        }
        _ => Ok(()),
    }
}

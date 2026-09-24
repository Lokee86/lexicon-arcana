use crate::{SnapshotManifest, StorageError};

use super::LanguageResult;

pub fn assemble_manifest(
    mut manifest: SnapshotManifest,
    mut results: Vec<LanguageResult>,
) -> Result<SnapshotManifest, StorageError> {
    results.sort_by(|left, right| left.language.cmp(&right.language));
    for result in results {
        match result.entry {
            Some(entry) if entry.language == result.language => {
                manifest = manifest.with_language(entry);
            }
            Some(entry) => {
                return Err(StorageError::Materialization(format!(
                    "analysis result for {:?} contained {:?}",
                    result.language, entry.language
                )));
            }
            None => manifest = manifest.without_language(&result.language),
        }
    }
    Ok(manifest)
}

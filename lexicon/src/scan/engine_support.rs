use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::languages::for_path;
use crate::{AdapterHost, SnapshotManifest};

use super::ScanExecutionError;

pub(crate) fn languages_in_tree(root: &Path) -> Result<Vec<String>, std::io::Error> {
    let mut languages = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                stack.push(path);
            } else {
                languages.extend(for_path(path.to_string_lossy().as_ref()));
            }
        }
    }
    Ok(languages.into_iter().collect())
}

pub(crate) fn adapter_fingerprints(
    host: &AdapterHost,
    manifest: &SnapshotManifest,
) -> Result<BTreeMap<String, String>, ScanExecutionError> {
    manifest
        .languages
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter(|entry| entry.language != "interstack")
        .map(|entry| {
            host.fingerprint(&entry.language)
                .map(|fingerprint| (entry.language.clone(), fingerprint))
                .map_err(ScanExecutionError::from)
        })
        .collect()
}

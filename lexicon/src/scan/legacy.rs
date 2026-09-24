use std::fs;
use std::path::Path;

use crate::{ANALYSIS_CONFIG_ID, AdapterHost, SnapshotManifest, Store, adapter_fingerprint};

use super::ScanExecutionError;
use super::legacy_parse::parse_legacy_analysis;
use super::sources::language_sources;

pub(crate) fn build_legacy_manifest(
    store: &Store,
    state_root: &Path,
    state_commit: &str,
    host: &AdapterHost,
) -> Result<SnapshotManifest, ScanExecutionError> {
    let library_root = state_root.join("library");
    let mut entries = match fs::read_dir(&library_root) {
        Ok(entries) => entries.collect::<Result<Vec<_>, _>>()?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    entries.sort_by_key(fs::DirEntry::file_name);

    let mut languages = Vec::new();
    for entry in entries {
        if entry.file_type()?.is_dir()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("jsonl")
        {
            continue;
        }
        let language = entry
            .path()
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| ScanExecutionError::new("legacy Lexicon library name is not UTF-8"))?
            .to_owned();
        let input = fs::read_to_string(entry.path())?;
        let analysis = parse_legacy_analysis(&input, &language)?;
        let sources = language_sources(&state_root.join("source"), &language)?;
        let fingerprint = if host.root().as_os_str().is_empty() {
            String::new()
        } else {
            adapter_fingerprint(host.root(), &language)?
        };
        let language_entry = store.build_full_language(
            &analysis,
            &sources,
            &language,
            ANALYSIS_CONFIG_ID,
            &fingerprint,
        )?;
        languages.push(language_entry);
    }
    languages.sort_by(|left, right| left.language.cmp(&right.language));

    Ok(SnapshotManifest {
        version: crate::storage::SNAPSHOT_VERSION,
        state_commit: state_commit.to_owned(),
        languages: Some(languages),
    })
}

pub(crate) fn legacy_library_exists(state_root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(state_root.join("library")) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        !entry.file_type().is_ok_and(|kind| kind.is_dir())
            && entry.path().extension().and_then(|value| value.to_str()) == Some("jsonl")
    })
}

pub(crate) fn remove_legacy_library(state_root: &Path) -> Result<bool, std::io::Error> {
    let path = state_root.join("library");
    let metadata = match fs::metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if metadata.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(true)
}

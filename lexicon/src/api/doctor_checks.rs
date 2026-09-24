use std::path::Path;

use crate::{
    SnapshotManifest, StateRepository, Store, list_consumer_paths, load_consumer_definition,
};

use super::DoctorReport;
use super::doctor_runtime::check_command;

pub(super) fn verify_state_repository(path: &Path) -> Result<(), String> {
    let repository = StateRepository::open(path).map_err(|error| error.to_string())?;
    repository
        .head()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub(super) fn verify_snapshot(store: &Store) -> (SnapshotManifest, Option<String>) {
    let (_, manifest) = match store.current() {
        Ok(current) => current,
        Err(error) => return (empty_manifest(), Some(error.to_string())),
    };
    let mut failures = Vec::new();
    for id in referenced_object_ids(&manifest) {
        if let Err(error) = store.load_object(&id) {
            failures.push(format!("snapshot object {id}: {error}"));
        }
    }
    let error = (!failures.is_empty()).then(|| failures.join("; "));
    (manifest, error)
}

pub(super) fn manifest_languages(manifest: &SnapshotManifest) -> Vec<String> {
    let mut languages = std::collections::BTreeSet::new();
    for entry in manifest.languages.as_deref().unwrap_or_default() {
        match entry.language.as_str() {
            "interstack" => {}
            language if language.starts_with("generic-") => {
                languages.insert("generic".to_owned());
            }
            language => {
                languages.insert(language.to_owned());
            }
        }
    }
    languages.into_iter().collect()
}

pub(super) fn inspect_consumers(repository: &Path, report: &mut DoctorReport) {
    let root = repository.join(".lexicon");
    let paths = match list_consumer_paths(&root) {
        Ok(paths) => paths,
        Err(error) => {
            report.fail("registered consumer definitions", error.to_string());
            return;
        }
    };
    if paths.is_empty() {
        report.pass("registered consumer definitions");
        return;
    }
    for path in paths {
        inspect_consumer(&path, report);
    }
}

fn inspect_consumer(path: &Path, report: &mut DoctorReport) {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("<invalid>")
        .to_owned();
    match load_consumer_definition(path) {
        Ok(definition) => {
            report.pass(format!("consumer definition: {name}"));
            match check_command(&definition.command) {
                Ok(()) => report.pass(format!("consumer command: {name}")),
                Err(error) => report.fail(format!("consumer command: {name}"), error),
            }
        }
        Err(error) => {
            report.fail(format!("consumer definition: {name}"), error.to_string());
            report.fail(
                format!("consumer command: {name}"),
                "definition unavailable",
            );
        }
    }
}

fn referenced_object_ids(manifest: &SnapshotManifest) -> Vec<String> {
    let mut ids = std::collections::BTreeSet::new();
    for language in manifest.languages.as_deref().unwrap_or_default() {
        if !language.shared_object_id.is_empty() {
            ids.insert(language.shared_object_id.clone());
        }
        for file in language.files.as_deref().unwrap_or_default() {
            ids.insert(file.object_id.clone());
        }
    }
    ids.into_iter().collect()
}

fn empty_manifest() -> SnapshotManifest {
    SnapshotManifest {
        version: crate::storage::SNAPSHOT_VERSION,
        state_commit: String::new(),
        languages: Some(Vec::new()),
    }
}

#![allow(dead_code)]

use std::fs;
use std::sync::Mutex;

use lexicon::{
    AdapterError, AdapterRequest, Analysis, AnalysisPlan, FactHeader, LanguageAdapter,
    SnapshotManifest,
};

pub struct FixtureAdapter {
    pub requests: Mutex<Vec<bool>>,
    fail_scoped: bool,
}

impl FixtureAdapter {
    pub fn new(fail_scoped: bool) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            fail_scoped,
        }
    }
}

impl LanguageAdapter for FixtureAdapter {
    fn implementation_version(&self) -> &'static str {
        "test"
    }
    fn implementation_fingerprint(&self) -> String {
        "fixture".into()
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        let scoped = !request.changed_files.is_empty();
        self.requests.lock().unwrap().push(scoped);
        if scoped && self.fail_scoped {
            return Err(AdapterError::from(std::io::Error::other(
                "scoped repository is incomplete",
            )));
        }
        let header = if scoped {
            incremental_header(&request.language, &request.changed_files)
        } else {
            full_header(&request.language)
        };
        Ok(Analysis::new(header, Vec::new()))
    }
}

pub fn full_plan() -> AnalysisPlan {
    AnalysisPlan {
        language: "python".into(),
        full: true,
        known_present: true,
        changed_files: Vec::new(),
        removed_files: Vec::new(),
        context_files: Vec::new(),
    }
}

pub fn incremental_plan() -> AnalysisPlan {
    AnalysisPlan {
        language: "python".into(),
        full: false,
        known_present: false,
        changed_files: vec!["a.py".into()],
        removed_files: Vec::new(),
        context_files: vec!["a.py".into()],
    }
}

pub fn empty_manifest() -> SnapshotManifest {
    SnapshotManifest {
        version: 1,
        state_commit: String::new(),
        languages: Some(Vec::new()),
    }
}

pub fn file_object(entry: &lexicon::LanguageEntry, path: &str) -> String {
    entry
        .files
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|file| file.path == path)
        .unwrap()
        .object_id
        .clone()
}

pub fn paths(entry: &lexicon::LanguageEntry) -> Vec<String> {
    entry
        .files
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|file| file.path.clone())
        .collect()
}

pub fn write(root: &std::path::Path, relative: &str, data: &str) {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}

fn full_header(language: &str) -> FactHeader {
    FactHeader {
        adapter_version: "test".into(),
        changed_files: None,
        language: language.into(),
        mode: None,
        record: "lexicon".into(),
        removed_files: None,
        repository: "repo".into(),
        schema_version: 1,
        shared_complete: None,
    }
}

fn incremental_header(language: &str, changed: &[String]) -> FactHeader {
    FactHeader {
        adapter_version: "test".into(),
        changed_files: Some(changed.to_vec()),
        language: language.into(),
        mode: Some("incremental".into()),
        record: "lexicon".into(),
        removed_files: Some(Vec::new()),
        repository: "repo".into(),
        schema_version: 1,
        shared_complete: Some(true),
    }
}

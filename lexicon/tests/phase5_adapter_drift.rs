mod support;

use std::fs;
use std::sync::{Arc, Mutex};

use lexicon::{
    AdapterError, AdapterHost, AdapterRequest, Analysis, FactHeader, LanguageAdapter, ScanEngine,
    StateRepository, Store,
};

use support::TestDirectory;

struct FixtureAdapter {
    scoped_requests: Mutex<Vec<bool>>,
}

impl FixtureAdapter {
    fn new() -> Self {
        Self {
            scoped_requests: Mutex::new(Vec::new()),
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
        self.scoped_requests.lock().unwrap().push(scoped);
        let header = if scoped {
            FactHeader {
                adapter_version: "test".into(),
                changed_files: Some(request.changed_files.clone()),
                language: request.language.clone(),
                mode: Some("incremental".into()),
                record: "lexicon".into(),
                removed_files: Some(Vec::new()),
                repository: "repo".into(),
                schema_version: 1,
                shared_complete: Some(true),
            }
        } else {
            FactHeader {
                adapter_version: "test".into(),
                changed_files: None,
                language: request.language.clone(),
                mode: None,
                record: "lexicon".into(),
                removed_files: None,
                repository: "repo".into(),
                schema_version: 1,
                shared_complete: None,
            }
        };
        Ok(Analysis::new(header, Vec::new()))
    }
}

#[test]
fn adapter_fingerprint_drift_forces_full_scan_without_source_changes() {
    let root = TestDirectory::new("phase5-adapter-drift");
    let repository = root.path.join("repository");
    let state_root = root.path.join("state");
    let store_root = root.path.join("store");
    let adapter_root = root.path.join("adapters");

    write(&repository, "a.py", "value = 1\n");
    let git = StateRepository::ensure(&state_root).unwrap();
    let adapter = Arc::new(FixtureAdapter::new());
    let mut host = AdapterHost::new(&adapter_root);
    host.register("python", adapter.clone());
    let engine = ScanEngine::new(&repository, git, Store::new(&store_root), host, Vec::new());

    let initial = engine.scan().unwrap();
    let (_, mut previous) = engine.store().current().unwrap();
    let mut stale = previous.language("python").unwrap().clone();
    // A previously installed adapter can change independently of source.
    // A legacy entry has no index to invalidate with its stale fingerprint.
    stale.adapter_fingerprint = "sha256:outdated-adapter".into();
    stale.dependency_index_id.clear();
    previous = previous.with_language(stale);
    let outdated = engine.store().publish(&previous).unwrap();
    assert_ne!(outdated, initial.snapshot_id);

    let result = engine.scan().unwrap();
    assert!(result.changed.is_empty());
    assert_eq!(result.languages, vec!["python"]);
    assert_ne!(result.snapshot_id, outdated);
    assert_eq!(*adapter.scoped_requests.lock().unwrap(), vec![false, false]);
    let (_, updated) = engine.store().current().unwrap();
    let python = updated.language("python").unwrap();
    assert_ne!(python.adapter_fingerprint, "sha256:outdated-adapter");
    assert!(!python.dependency_index_id.is_empty());
}

fn write(root: &std::path::Path, relative: &str, data: &str) {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}

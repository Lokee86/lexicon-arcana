mod support;

use std::fs;
use std::io::Write;
use std::sync::{Arc, Mutex};

use lexicon::{
    AdapterError, AdapterHost, AdapterRequest, NativeAdapter, ScanEngine, StateRepository, Store,
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

impl NativeAdapter for FixtureAdapter {
    fn run(&self, request: &AdapterRequest, output: &mut dyn Write) -> Result<(), AdapterError> {
        let scoped = !request.changed_files.is_empty();
        self.scoped_requests.lock().unwrap().push(scoped);
        if scoped {
            writeln!(
                output,
                "{{\"adapter_version\":\"test\",\"changed_files\":{},\"language\":\"{}\",\"mode\":\"incremental\",\"record\":\"lexicon\",\"removed_files\":[],\"repository\":\"repo\",\"schema_version\":1,\"shared_complete\":true}}",
                serde_json::to_string(&request.changed_files).unwrap(),
                request.language,
            )
        } else {
            writeln!(
                output,
                "{{\"adapter_version\":\"test\",\"language\":\"{}\",\"record\":\"lexicon\",\"repository\":\"repo\",\"schema_version\":1}}",
                request.language,
            )
        }
        .map_err(AdapterError::from)
    }
}

#[test]
fn repository_scan_runs_full_noop_then_incremental_transaction() {
    let root = TestDirectory::new("scan-engine");
    let repository = root.path.join("repository");
    let state_root = root.path.join("state");
    let store_root = root.path.join("store");
    let adapter_root = root.path.join("adapters");

    write(&repository, "a.py", "value = 1\n");
    write(&repository, "pyproject.toml", "[project]\nname='sample'\n");
    write(&adapter_root, "python/adapter.py", "version = 1\n");

    let git = StateRepository::ensure(&state_root).unwrap();
    let adapter = Arc::new(FixtureAdapter::new());
    let mut host = AdapterHost::new(&adapter_root);
    host.register_native("python", adapter.clone());
    let engine = ScanEngine::new(&repository, git, Store::new(&store_root), host, Vec::new());

    let first = engine.scan().unwrap();
    assert_eq!(first.languages, vec!["python"]);
    assert!(first.changed.is_empty());
    let first_head = engine.state_repository().head().unwrap();

    let second = engine.scan().unwrap();
    assert_eq!(second.snapshot_id, first.snapshot_id);
    assert!(second.changed.is_empty());
    assert!(second.languages.is_empty());
    assert_eq!(engine.state_repository().head().unwrap(), first_head);

    write(&repository, "a.py", "value = 2\n");
    let third = engine.scan().unwrap();
    assert_ne!(third.snapshot_id, first.snapshot_id);
    assert_eq!(third.languages, vec!["python"]);
    assert_eq!(third.changed.len(), 1);
    assert_eq!(third.changed[0].status, "M");
    assert_eq!(third.changed[0].new, "a.py");
    assert_ne!(engine.state_repository().head().unwrap(), first_head);
    assert_eq!(*adapter.scoped_requests.lock().unwrap(), vec![false, true]);

    let (_, current) = engine.store().current().unwrap();
    assert_eq!(
        current.state_commit,
        engine.state_repository().head().unwrap()
    );
    assert!(current.language("python").is_some());
}

#[test]
fn interstack_drift_forces_refresh_without_ordinary_adapter_work() {
    let root = TestDirectory::new("scan-engine-interstack-drift");
    let repository = root.path.join("repository");
    let state_root = root.path.join("state");
    let store_root = root.path.join("store");
    let adapter_root = root.path.join("adapters");

    write(&repository, "a.py", "value = 1\n");
    write(&adapter_root, "python/adapter.py", "version = 1\n");

    let git = StateRepository::ensure(&state_root).unwrap();
    let adapter = Arc::new(FixtureAdapter::new());
    let mut host = AdapterHost::new(&adapter_root);
    host.register_native("python", adapter.clone());
    let engine = ScanEngine::new(&repository, git, Store::new(&store_root), host, Vec::new());

    let first = engine.scan().unwrap();
    let (_, mut stale_manifest) = engine.store().current().unwrap();
    let mut stale_interstack = stale_manifest
        .language(lexicon::interstack::LANGUAGE)
        .expect("interstack entry")
        .clone();
    stale_interstack.adapter_fingerprint = format!("sha256:{}", "0".repeat(64));
    stale_manifest = stale_manifest.with_language(stale_interstack);
    let stale_snapshot = engine.store().publish(&stale_manifest).unwrap();
    assert_ne!(stale_snapshot, first.snapshot_id);

    let refreshed = engine.scan().unwrap();
    assert!(refreshed.changed.is_empty());
    assert!(refreshed.languages.is_empty());
    assert_ne!(refreshed.snapshot_id, stale_snapshot);
    assert_eq!(*adapter.scoped_requests.lock().unwrap(), vec![false]);

    let (_, current) = engine.store().current().unwrap();
    assert_eq!(
        current
            .language(lexicon::interstack::LANGUAGE)
            .unwrap()
            .adapter_fingerprint,
        lexicon::interstack::adapter_fingerprint()
    );
}

fn write(root: &std::path::Path, relative: &str, data: &str) {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}

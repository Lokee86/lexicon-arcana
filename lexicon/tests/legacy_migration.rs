mod support;

use std::sync::Arc;

use lexicon::{AdapterHost, ScanEngine, StateRepository, Store};

use support::TestDirectory;
use support::scan_adapter::{FixtureAdapter, write};

#[test]
fn migrates_committed_legacy_library_ahead_of_current_snapshot() {
    let fixture = Fixture::new("legacy-ahead");
    let (engine, adapter) = fixture.engine();

    let first = engine.scan().unwrap();
    let base_head = engine.state_repository().head().unwrap();
    assert_eq!(*adapter.requests.lock().unwrap(), vec![false]);

    write_legacy(&fixture.state_root, valid_legacy());
    engine.state_repository().stage_all().unwrap();
    engine.state_repository().commit_state().unwrap();
    let legacy_head = engine.state_repository().head().unwrap();
    assert_ne!(legacy_head, base_head);

    let migrated = engine.scan().unwrap();
    assert_ne!(migrated.snapshot_id, first.snapshot_id);
    assert!(migrated.languages.is_empty());
    assert!(!fixture.state_root.join("library").exists());
    assert_eq!(*adapter.requests.lock().unwrap(), vec![false]);

    let (_, current) = engine.store().current().unwrap();
    assert_eq!(
        current.state_commit,
        engine.state_repository().head().unwrap()
    );
    assert!(current.language("python").is_some());
}

#[test]
fn migrates_legacy_library_when_current_snapshot_is_absent() {
    let fixture = Fixture::new("legacy-no-current");
    fixture.prepare_committed_legacy(valid_legacy());
    let (engine, adapter) = fixture.engine();

    let report = engine.scan().unwrap();

    assert!(report.languages.is_empty());
    assert!(!fixture.state_root.join("library").exists());
    assert!(adapter.requests.lock().unwrap().is_empty());
    let (_, current) = engine.store().current().unwrap();
    assert_eq!(
        current.state_commit,
        engine.state_repository().head().unwrap()
    );
    assert!(current.language("python").is_some());
}

#[test]
fn corrupt_legacy_library_is_removed_and_rebuilt_from_source() {
    let fixture = Fixture::new("legacy-corrupt");
    fixture.prepare_committed_legacy("{not-json}\n");
    let (engine, adapter) = fixture.engine();

    let report = engine.scan().unwrap();

    assert_eq!(report.languages, vec!["python"]);
    assert!(!fixture.state_root.join("library").exists());
    assert_eq!(*adapter.requests.lock().unwrap(), vec![false]);
    let (_, current) = engine.store().current().unwrap();
    assert!(current.language("python").is_some());
}

struct Fixture {
    _root: TestDirectory,
    repository: std::path::PathBuf,
    state_root: std::path::PathBuf,
    store_root: std::path::PathBuf,
    adapter_root: std::path::PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = TestDirectory::new(name);
        let repository = root.path.join("repository");
        let state_root = root.path.join("state");
        let store_root = root.path.join("store");
        let adapter_root = root.path.join("adapters");
        write(&repository, "a.py", "value = 1\n");
        write(&adapter_root, "python/adapter.py", "version = 1\n");
        Self {
            _root: root,
            repository,
            state_root,
            store_root,
            adapter_root,
        }
    }

    fn engine(&self) -> (ScanEngine, Arc<FixtureAdapter>) {
        let git = match StateRepository::open(&self.state_root) {
            Ok(git) => git,
            Err(_) => StateRepository::ensure(&self.state_root).unwrap(),
        };
        let adapter = Arc::new(FixtureAdapter::new(false));
        let mut host = AdapterHost::new(&self.adapter_root);
        host.register_native("python", adapter.clone());
        (
            ScanEngine::new(
                &self.repository,
                git,
                Store::new(&self.store_root),
                host,
                Vec::new(),
            ),
            adapter,
        )
    }

    fn prepare_committed_legacy(&self, legacy: &str) {
        let git = StateRepository::ensure(&self.state_root).unwrap();
        write(&self.state_root, "source/a.py", "value = 1\n");
        write_legacy(&self.state_root, legacy);
        git.stage_all().unwrap();
        git.commit_state().unwrap();
    }
}

fn write_legacy(state_root: &std::path::Path, body: &str) {
    write(state_root, "library/python.jsonl", body);
}

fn valid_legacy() -> &'static str {
    "{\"adapter_version\":\"test\",\"language\":\"python\",\"mode\":\"full\",\"record\":\"lexicon\",\"repository\":\"repo\",\"schema_version\":1}\n\
{\"id\":\"a\",\"kind\":\"file\",\"name\":\"a.py\",\"owner\":\"a.py\",\"path\":\"a.py\",\"qualified_name\":\"a.py\",\"record\":\"node\"}\n"
}

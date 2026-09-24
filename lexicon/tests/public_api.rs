mod support;

use std::sync::Arc;

use lexicon::{AdapterHost, GcOptions, Lexicon, load_config};

use support::TestDirectory;
use support::scan_adapter::{FixtureAdapter, write};

#[test]
fn initialize_open_and_scan_use_public_library_handle() {
    let fixture = Fixture::new("public-api");
    let adapter = Arc::new(FixtureAdapter::new(false));
    let host = fixture.host(adapter.clone());

    let (lexicon, report) = Lexicon::initialize_with_host(&fixture.repository, host).unwrap();

    assert_eq!(report.languages, vec!["python"]);
    assert!(report.changed.is_empty());
    assert_eq!(*adapter.requests.lock().unwrap(), vec![false]);
    assert_eq!(lexicon.repository(), fixture.repository.as_path());
    assert_eq!(
        lexicon.state_root(),
        fixture.repository.join(".lexicon").as_path()
    );
    assert_eq!(lexicon.adapter_root(), fixture.adapter_root.as_path());

    let current = lexicon.scan().unwrap();
    assert_eq!(current.snapshot_id, report.snapshot_id);
    assert!(current.languages.is_empty());
    assert_eq!(*adapter.requests.lock().unwrap(), vec![false]);

    let opened = Lexicon::open(&fixture.repository).unwrap();
    assert_eq!(opened.repository(), fixture.repository.as_path());
    assert_eq!(opened.adapter_root(), fixture.adapter_root.as_path());
    assert_eq!(opened.store().current().unwrap().0, report.snapshot_id);

    let export = fixture.repository.join("export");
    opened.export("CURRENT", &export, &[]).unwrap();
    assert!(export.join("python.jsonl").exists());

    let gc = opened
        .garbage_collect(GcOptions { keep_snapshots: 1 }, true)
        .unwrap();
    assert!(gc.dry_run);
}

#[test]
fn reinitialize_forces_full_analysis_and_preserves_configured_languages() {
    let fixture = Fixture::new("public-api-reinitialize");
    write(&fixture.repository, "main.go", "package main\n");
    write(&fixture.adapter_root, "go/adapter.go", "version = 1\n");

    let adapter = Arc::new(FixtureAdapter::new(false));
    let host = fixture.host(adapter.clone());
    let enabled = vec!["python".to_owned()];
    let (_, first) =
        Lexicon::initialize_with_host_and_languages(&fixture.repository, host, &enabled).unwrap();
    assert_eq!(first.languages, vec!["python"]);
    assert_eq!(*adapter.requests.lock().unwrap(), vec![false]);

    let host = fixture.host(adapter.clone());
    let (_, second) = Lexicon::initialize_with_host(&fixture.repository, host).unwrap();
    assert_eq!(second.languages, vec!["python"]);
    assert_eq!(*adapter.requests.lock().unwrap(), vec![false, false]);

    let config = load_config(&fixture.repository).unwrap();
    assert_eq!(config.enabled_languages, vec!["python"]);
}

#[test]
fn open_requires_initialized_configuration() {
    let root = TestDirectory::new("public-api-missing-config");
    let repository = root.path.join("repository");
    std::fs::create_dir_all(&repository).unwrap();

    let error = Lexicon::open(&repository).err().expect("open should fail");
    assert!(error.to_string().contains("read Lexicon configuration"));
}

struct Fixture {
    _root: TestDirectory,
    repository: std::path::PathBuf,
    adapter_root: std::path::PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = TestDirectory::new(name);
        let repository = root.path.join("repository");
        let adapter_root = root.path.join("adapters");
        write(&repository, "a.py", "value = 1\n");
        write(&adapter_root, "python/adapter.py", "version = 1\n");
        Self {
            _root: root,
            repository,
            adapter_root,
        }
    }

    fn host(&self, adapter: Arc<FixtureAdapter>) -> AdapterHost {
        let mut host = AdapterHost::new(&self.adapter_root);
        host.register("python", adapter);
        host
    }
}

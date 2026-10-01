use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use lexicon::{
    ConsumerDefinition, LanguageEntry, SnapshotManifest, Store, add_consumer_definition,
    load_consumer_definition, save_config, state_root,
};
use lexicon_cli::run;

#[test]
fn status_languages_and_consumer_registry_match_host_contract() {
    let fixture = Fixture::new("cli-status");

    let (code, stdout, stderr) = invoke(["status", "--repo", fixture.repository.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains(&format!(
        "repository root: {}",
        fixture.repository.canonicalize().unwrap().display()
    )));
    assert!(stdout.contains("current snapshot ID: sha256:"));
    assert!(stdout.contains("detected languages: go, python"));
    assert!(stdout.contains("enabled languages: all"));
    assert!(stdout.contains("registered consumer names: alpha"));

    let (code, stdout, stderr) =
        invoke(["languages", "--repo", fixture.repository.to_str().unwrap()]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, "enabled languages: all\n");

    let (code, stdout, stderr) = invoke([
        "consumer",
        "list",
        "--repo",
        fixture.repository.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, "alpha\n");
}

#[test]
fn consumer_add_and_remove_use_library_registry() {
    let fixture = Fixture::new("cli-consumer");

    let (code, stdout, stderr) = invoke([
        "consumer",
        "add",
        "--repo",
        fixture.repository.to_str().unwrap(),
        "--name",
        "beta",
        "--command",
        "example-consumer",
        "--arg",
        "sync",
        "--timeout",
        "2s",
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, "registered consumer: beta\n");

    let definition = load_consumer_definition(
        &state_root(&fixture.repository)
            .join("consumers")
            .join("beta.json"),
    )
    .unwrap();
    assert_eq!(definition.command, "example-consumer");
    assert_eq!(definition.args, vec!["sync"]);
    assert_eq!(definition.timeout_nanos, 2_000_000_000);

    let (code, stdout, stderr) = invoke([
        "consumer",
        "remove",
        "--repo",
        fixture.repository.to_str().unwrap(),
        "--name",
        "beta",
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, "removed consumer: beta\n");
    assert!(
        !state_root(&fixture.repository)
            .join("consumers")
            .join("beta.json")
            .exists()
    );
}

#[test]
fn gc_dry_run_and_required_option_errors_are_host_owned() {
    let fixture = Fixture::new("cli-gc");
    let (code, stdout, stderr) = invoke([
        "gc",
        "--repo",
        fixture.repository.to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(stdout, "would delete 0 snapshots and 0 objects\n");

    let (code, _, stderr) = invoke(["export", "--repo", fixture.repository.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(stderr.contains("export requires --output"));

    let (code, _, stderr) = invoke([
        "consumer",
        "add",
        "--repo",
        fixture.repository.to_str().unwrap(),
        "--name",
        "missing-command",
    ]);
    assert_eq!(code, 1);
    assert!(stderr.contains("consumer add requires --name and --command"));
}

fn invoke<const N: usize>(arguments: [&str; N]) -> (i32, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = run(
        arguments.into_iter().map(str::to_owned).collect(),
        &mut stdout,
        &mut stderr,
    );
    (
        code,
        String::from_utf8(stdout).unwrap(),
        String::from_utf8(stderr).unwrap(),
    )
}

struct Fixture {
    _root: TestDir,
    repository: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = TestDir::new(name);
        let repository = root.path().join("repository");
        let adapters = root.path().join("adapters");
        fs::create_dir_all(&repository).unwrap();
        fs::create_dir_all(&adapters).unwrap();
        save_config(&repository, &adapters).unwrap();

        let state = state_root(&repository);
        let store = Store::new(&state);
        store
            .publish(&SnapshotManifest {
                version: 1,
                state_commit: "state-commit".into(),
                languages: Some(vec![language("python"), language("go")]),
            })
            .unwrap();

        add_consumer_definition(
            &state,
            "alpha.json",
            &ConsumerDefinition {
                version: 1,
                command: "example".into(),
                args: Vec::new(),
                timeout_nanos: 0,
            },
        )
        .unwrap();

        Self {
            _root: root,
            repository,
        }
    }
}

fn language(name: &str) -> LanguageEntry {
    LanguageEntry {
        language: name.into(),
        adapter_version: String::new(),
        adapter_fingerprint: String::new(),
        schema_version: 0,
        repository: String::new(),
        analysis_config_id: String::new(),
        shared_object_id: String::new(),
        dependency_index_id: String::new(),
        files: Some(Vec::new()),
    }
}

struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lexicon-cli-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

mod support;

use std::fs;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use lexicon::{
    AdapterHost, CONSUMER_STATE_VERSION, ConsumerDefinition, ConsumerSuccessState, Lexicon,
    add_consumer_definition, run_consumer, run_consumers, state_root, timeout_for,
    validate_consumer_name,
};

use support::TestDirectory;
use support::scan_adapter::{FixtureAdapter, write};

const SNAPSHOT: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";

#[test]
fn runs_all_consumers_in_order_aggregates_failures_and_persists_successes() {
    let root = TestDirectory::new("consumer-run-all");
    let repository = root.path.join("repository");
    let state = repository.join(".lexicon");
    fs::create_dir_all(&repository).unwrap();

    add(&state, "01-failing.json", "helper_fail", 0);
    add(&state, "02-success.json", "helper_success_02", 0);
    add(&state, "03-success.json", "helper_success_03", 0);

    let mut output = Vec::new();
    let error = run_consumers(&repository, &state, SNAPSHOT, Some(&mut output)).unwrap_err();
    assert!(error.to_string().contains("01-failing.json"));

    let order = fs::read_to_string(repository.join("consumer-order")).unwrap();
    assert_eq!(order, "fail\nsuccess-02\nsuccess-03\n");
    assert!(
        String::from_utf8(output)
            .unwrap()
            .contains("helper-output-success-02")
    );

    for name in ["02-success.json", "03-success.json"] {
        let data = fs::read(state.join("consumer-state").join(name)).unwrap();
        let success: ConsumerSuccessState = serde_json::from_slice(&data).unwrap();
        assert_eq!(success.version, CONSUMER_STATE_VERSION);
        assert_eq!(success.snapshot_id, SNAPSHOT);
        assert_eq!(
            String::from_utf8(data).unwrap(),
            format!("{{\n  \"version\": 1,\n  \"snapshot_id\": \"{SNAPSHOT}\"\n}}\n")
        );
    }
    assert!(!state.join("consumer-state/01-failing.json").exists());
}

#[test]
fn consumer_receives_repository_state_snapshot_and_working_directory() {
    let root = TestDirectory::new("consumer-environment");
    let repository = root.path.join("repository");
    let state = repository.join(".lexicon");
    fs::create_dir_all(&repository).unwrap();
    add(&state, "env.json", "helper_environment", 0);

    run_consumer(&repository, &state, "env.json", SNAPSHOT, None).unwrap();

    let value = fs::read_to_string(repository.join("consumer-environment")).unwrap();
    let lines = value.lines().collect::<Vec<_>>();
    assert_eq!(lines[0], repository.to_string_lossy());
    assert_eq!(lines[1], state.to_string_lossy());
    assert_eq!(lines[2], SNAPSHOT);
    assert_eq!(
        std::path::PathBuf::from(lines[3]).canonicalize().unwrap(),
        repository.canonicalize().unwrap()
    );
}

#[test]
fn timeout_is_bounded_and_does_not_persist_success_state() {
    let root = TestDirectory::new("consumer-timeout");
    let repository = root.path.join("repository");
    let state = repository.join(".lexicon");
    fs::create_dir_all(&repository).unwrap();
    add(&state, "slow.json", "helper_sleep", 10_000_000);

    let definition = lexicon::load_consumer_definition(&state.join("consumers/slow.json")).unwrap();
    assert_eq!(timeout_for(&definition), Duration::from_millis(10));

    let error = run_consumer(&repository, &state, "slow.json", SNAPSHOT, None).unwrap_err();
    assert!(error.to_string().contains("context deadline exceeded"));
    assert!(!state.join("consumer-state/slow.json").exists());
}

#[test]
fn validates_names_and_registry_json_compatibility() {
    let root = TestDirectory::new("consumer-registry");
    let state = root.path.join(".lexicon");
    let definition = ConsumerDefinition {
        version: 1,
        command: "arcana".into(),
        args: vec!["sync".into()],
        timeout_nanos: 1_500_000,
    };
    add_consumer_definition(&state, "arcana.json", &definition).unwrap();
    assert_eq!(
        fs::read_to_string(state.join("consumers/arcana.json")).unwrap(),
        "{\n  \"version\": 1,\n  \"command\": \"arcana\",\n  \"args\": [\n    \"sync\"\n  ],\n  \"timeout\": \"1.5ms\"\n}\n"
    );

    for name in [
        "",
        "alpha",
        "../alpha.json",
        r"nested\alpha.json",
        "alpha.txt",
        ".json",
    ] {
        assert!(validate_consumer_name(name).is_err(), "accepted {name:?}");
    }
}

#[test]
fn public_scan_notifies_consumers_but_initialize_does_not() {
    let root = TestDirectory::new("consumer-scan-hook");
    let repository = root.path.join("repository");
    let adapter_root = root.path.join("adapters");
    write(&repository, "a.py", "value = 1\n");
    write(&adapter_root, "python/adapter.py", "version = 1\n");

    let state = state_root(&repository);
    add(&state, "scan.json", "helper_scan", 0);

    let adapter = Arc::new(FixtureAdapter::new(false));
    let mut host = AdapterHost::new(&adapter_root);
    host.register_native("python", adapter);

    let (lexicon, initialized) = Lexicon::initialize_with_host(&repository, host).unwrap();
    assert!(!repository.join("scan-consumer").exists());
    assert!(!state.join("consumer-state/scan.json").exists());

    let report = lexicon.scan().unwrap();
    assert_eq!(report.snapshot_id, initialized.snapshot_id);
    assert_eq!(
        fs::read_to_string(repository.join("scan-consumer")).unwrap(),
        report.snapshot_id
    );
    let success: ConsumerSuccessState =
        serde_json::from_slice(&fs::read(state.join("consumer-state/scan.json")).unwrap()).unwrap();
    assert_eq!(success.snapshot_id, report.snapshot_id);
}

fn add(state_root: &std::path::Path, name: &str, helper: &str, timeout_nanos: u64) {
    add_consumer_definition(
        state_root,
        name,
        &ConsumerDefinition {
            version: 1,
            command: std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            args: vec!["--exact".into(), helper.into(), "--nocapture".into()],
            timeout_nanos,
        },
    )
    .unwrap();
}

#[test]
fn helper_fail() {
    if !consumer_child() {
        return;
    }
    append_order("fail");
    std::process::exit(3);
}

#[test]
fn helper_success_02() {
    if !consumer_child() {
        return;
    }
    append_order("success-02");
    std::io::stdout()
        .write_all(b"helper-output-success-02")
        .unwrap();
    std::process::exit(0);
}

#[test]
fn helper_success_03() {
    if !consumer_child() {
        return;
    }
    append_order("success-03");
    std::process::exit(0);
}

#[test]
fn helper_environment() {
    if !consumer_child() {
        return;
    }
    let repository = std::env::var("LEXICON_REPOSITORY").unwrap();
    let state = std::env::var("LEXICON_STATE_ROOT").unwrap();
    let snapshot = std::env::var("LEXICON_SNAPSHOT_ID").unwrap();
    let cwd = std::env::current_dir().unwrap();
    fs::write(
        std::path::Path::new(&repository).join("consumer-environment"),
        format!("{repository}\n{state}\n{snapshot}\n{}\n", cwd.display()),
    )
    .unwrap();
    std::process::exit(0);
}

#[test]
fn helper_sleep() {
    if !consumer_child() {
        return;
    }
    std::thread::sleep(Duration::from_millis(200));
    std::process::exit(0);
}

#[test]
fn helper_scan() {
    if !consumer_child() {
        return;
    }
    let repository = std::env::var("LEXICON_REPOSITORY").unwrap();
    let snapshot = std::env::var("LEXICON_SNAPSHOT_ID").unwrap();
    fs::write(
        std::path::Path::new(&repository).join("scan-consumer"),
        snapshot,
    )
    .unwrap();
    std::process::exit(0);
}

fn consumer_child() -> bool {
    std::env::var_os("LEXICON_REPOSITORY").is_some()
        && std::env::var_os("LEXICON_STATE_ROOT").is_some()
        && std::env::var_os("LEXICON_SNAPSHOT_ID").is_some()
}

fn append_order(value: &str) {
    let repository = std::env::var("LEXICON_REPOSITORY").unwrap();
    let path = std::path::Path::new(&repository).join("consumer-order");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    writeln!(file, "{value}").unwrap();
}

mod support;

use std::fs;

use lexicon::{
    ANALYSIS_CONFIG_ID, FactObject, FileEntry, LanguageEntry, SnapshotManifest, StateRepository,
    Store, content_id, doctor, save_config, state_root,
};
use serde_json::json;

use support::TestDirectory;

#[test]
fn doctor_reports_passing_repository_checks_without_running_consumers() {
    let fixture = DoctorFixture::new("doctor-pass", &["go"]);
    fixture.create_go_runtime_helper();
    fixture.write_consumer(
        "arcana.json",
        json!({
            "version": 1,
            "command": std::env::current_exe().unwrap().to_string_lossy()
        }),
    );

    let report = doctor(&fixture.repository).unwrap();
    assert!(
        report.is_healthy(),
        "{:?}",
        report.failures().collect::<Vec<_>>()
    );
    for label in [
        "configuration loading",
        "private Git state repository",
        "CURRENT snapshot and referenced objects",
        "configured adapter root",
        "runtime helper: go",
        "consumer definition: arcana.json",
        "consumer command: arcana.json",
    ] {
        assert!(passed(&report, label), "missing PASS {label}");
    }
}

#[test]
fn doctor_maps_generic_libraries_and_skips_interstack() {
    let fixture = DoctorFixture::new(
        "doctor-synthetic",
        &["generic-sh", "generic-sql", "interstack", "go"],
    );
    fixture.create_adapter_directory("generic");
    fixture.create_go_runtime_helper();

    let report = doctor(&fixture.repository).unwrap();
    assert!(passed(&report, "adapter directory: generic"));
    assert!(passed(&report, "runtime helper: go"));
    assert!(!has_label(&report, "adapter directory: generic-sh"));
    assert!(!has_label(&report, "adapter directory: interstack"));
}

#[test]
fn doctor_reports_missing_native_go_helper() {
    let fixture = DoctorFixture::new("doctor-go-helper-missing", &["go"]);

    let report = doctor(&fixture.repository).unwrap();
    assert!(failed(&report, "runtime helper: go"));
    let error = report
        .checks
        .iter()
        .find(|check| check.label == "runtime helper: go")
        .and_then(|check| check.error.as_deref())
        .unwrap();
    assert!(
        error.contains("semantic frontend executable not found"),
        "{error}"
    );
    assert!(error.contains("install the packaged frontend"), "{error}");
    assert!(error.contains("LEXICON_GO_SEMANTIC_HELPER"), "{error}");
}

#[test]
fn doctor_reports_missing_typescript_runtime_entrypoint() {
    let fixture = DoctorFixture::new("doctor-typescript-helper-missing", &["typescript"]);

    let report = doctor(&fixture.repository).unwrap();
    assert!(failed(&report, "runtime helper: typescript"));
    let error = report
        .checks
        .iter()
        .find(|check| check.label == "runtime helper: typescript")
        .and_then(|check| check.error.as_deref())
        .unwrap();
    assert!(
        error.contains("TypeScript adapter entrypoint not found"),
        "{error}"
    );
    assert!(error.contains("LEXICON_TYPESCRIPT_ADAPTER"), "{error}");
}

#[test]
fn doctor_checks_adapter_directories_without_runtime_requirements() {
    let fixture = DoctorFixture::new("doctor-native-adapters", &["java", "kotlin", "csharp"]);
    for language in ["java", "kotlin", "csharp"] {
        fixture.create_adapter_directory(language);
    }

    let report = doctor(&fixture.repository).unwrap();
    for language in ["java", "kotlin", "csharp"] {
        assert!(passed(&report, &format!("adapter directory: {language}")));
        assert!(!has_label(
            &report,
            &format!("runtime executable: {language}")
        ));
    }
}

#[test]
fn doctor_aggregates_snapshot_adapter_and_consumer_failures() {
    let fixture = DoctorFixture::new("doctor-failures", &["java"]);
    fixture.create_adapter_directory("java");
    let (_, manifest) = fixture.store.current().unwrap();
    let object = manifest.languages.as_ref().unwrap()[0]
        .files
        .as_ref()
        .unwrap()[0]
        .object_id
        .clone();
    fs::write(fixture.store.object_path(&object), b"corrupt\n").unwrap();

    fixture.write_consumer("bad.json", json!({"version": 1}));
    fixture.write_consumer(
        "missing.json",
        json!({"version": 1, "command": "definitely-missing-lexicon-consumer"}),
    );

    let report = doctor(&fixture.repository).unwrap();
    assert!(!report.is_healthy());
    for label in [
        "CURRENT snapshot and referenced objects",
        "consumer definition: bad.json",
        "consumer command: bad.json",
        "consumer command: missing.json",
    ] {
        assert!(
            failed(&report, label),
            "missing FAIL {label}: {:?}",
            report.checks
        );
    }
    assert!(passed(&report, "adapter directory: java"));
    assert!(passed(&report, "consumer definition: missing.json"));
}

struct DoctorFixture {
    _root: TestDirectory,
    repository: std::path::PathBuf,
    adapters: std::path::PathBuf,
    store: Store,
}

impl DoctorFixture {
    fn new(name: &str, languages: &[&str]) -> Self {
        let root = TestDirectory::new(name);
        let repository = root.path.join("repository");
        let adapters = root.path.join("adapters");
        fs::create_dir_all(&repository).unwrap();
        fs::create_dir_all(&adapters).unwrap();
        save_config(&repository, &adapters).unwrap();

        let state = state_root(&repository);
        let git = StateRepository::ensure(state.join("repo")).unwrap();
        git.commit_state().unwrap();
        let store = Store::new(&state);
        let entries = languages
            .iter()
            .map(|language| language_entry(&store, language))
            .collect();
        store
            .publish(&SnapshotManifest {
                version: 1,
                state_commit: git.head().unwrap(),
                languages: Some(entries),
            })
            .unwrap();
        Self {
            _root: root,
            repository,
            adapters,
            store,
        }
    }

    fn create_adapter_directory(&self, language: &str) {
        fs::create_dir_all(self.adapters.join(language)).unwrap();
    }

    fn create_go_runtime_helper(&self) {
        let name = if cfg!(windows) {
            "lexicon-go-semantic.exe"
        } else {
            "lexicon-go-semantic"
        };
        let path = self.adapters.join("go-semantic").join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"helper").unwrap();
    }

    fn write_consumer(&self, name: &str, value: serde_json::Value) {
        let directory = self.repository.join(".lexicon").join("consumers");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(name), serde_json::to_vec(&value).unwrap()).unwrap();
    }
}

fn language_entry(store: &Store, language: &str) -> LanguageEntry {
    if language == "interstack" || language.starts_with("generic-") {
        return LanguageEntry {
            language: language.into(),
            adapter_version: "test".into(),
            adapter_fingerprint: String::new(),
            schema_version: 1,
            repository: "repo".into(),
            analysis_config_id: ANALYSIS_CONFIG_ID.into(),
            shared_object_id: String::new(),
            files: Some(Vec::new()),
        };
    }
    let owner = format!("main.{language}");
    let object = store
        .write_object(&FactObject {
            version: 1,
            language: language.into(),
            owner: owner.clone(),
            source_content_id: content_id(owner.as_bytes()),
            adapter_version: "test".into(),
            schema_version: 1,
            analysis_config_id: ANALYSIS_CONFIG_ID.into(),
            records: Vec::new(),
        })
        .unwrap();
    LanguageEntry {
        language: language.into(),
        adapter_version: "test".into(),
        adapter_fingerprint: String::new(),
        schema_version: 1,
        repository: "repo".into(),
        analysis_config_id: ANALYSIS_CONFIG_ID.into(),
        shared_object_id: String::new(),
        files: Some(vec![FileEntry {
            path: owner,
            language: language.into(),
            content_id: String::new(),
            object_id: object,
        }]),
    }
}

fn passed(report: &lexicon::DoctorReport, label: &str) -> bool {
    report
        .checks
        .iter()
        .any(|check| check.label == label && check.passed())
}

fn failed(report: &lexicon::DoctorReport, label: &str) -> bool {
    report
        .checks
        .iter()
        .any(|check| check.label == label && !check.passed())
}

fn has_label(report: &lexicon::DoctorReport, label: &str) -> bool {
    report.checks.iter().any(|check| check.label == label)
}

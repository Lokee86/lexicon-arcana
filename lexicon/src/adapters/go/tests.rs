use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::{AdapterHost, AdapterRequest, LanguageAdapter, ScanEngine, StateRepository, Store};

use super::{GoAdapter, helper_arguments, helper_environment};
use crate::adapters::helper::HelperRunner;

#[test]
fn adapter_host_registers_go() {
    let root = TempDirectory::new("host");
    let host = AdapterHost::new(&root.path);
    assert!(host.has_adapter("go"));
    assert!(host.fingerprint("go").unwrap().starts_with("sha256:"));
}

#[test]
fn native_go_adapter_owns_full_and_incremental_scan_path() {
    let root = TempDirectory::new("cutover-scan");
    let repository = root.path.join("repository");
    let state_root = root.path.join("state");
    let store_root = root.path.join("store");
    let adapter_root = root.path.join("adapters");
    fs::create_dir_all(repository.join("a")).unwrap();
    fs::create_dir_all(repository.join("b")).unwrap();
    fs::create_dir_all(&adapter_root).unwrap();
    fs::write(
        repository.join("go.mod"),
        "module example.com/cutover\n\ngo 1.22\n",
    )
    .unwrap();
    fs::write(
        repository.join("a").join("a.go"),
        "package a\n\nfunc Value() int { return 1 }\n",
    )
    .unwrap();
    fs::write(
        repository.join("b").join("b.go"),
        "package b\n\nfunc Stable() int { return 2 }\n",
    )
    .unwrap();

    let helper = synthetic_helper(&root.path, r#"{"protocol_version":1,"records":[]}"#);
    let mut host = AdapterHost::new(&adapter_root);
    host.register("go", Arc::new(GoAdapter::with_helper(helper)));
    let git = StateRepository::ensure(&state_root).unwrap();
    let engine = ScanEngine::new(
        &repository,
        git,
        Store::new(&store_root),
        host,
        vec!["go".into()],
    );

    let full = engine.scan().unwrap();
    assert_eq!(full.languages, vec!["go"]);
    let (_, first_manifest) = engine.store().current().unwrap();
    let first_go = first_manifest.language("go").unwrap();
    let first_files = first_go.files.as_ref().unwrap();
    let first_a = first_files
        .iter()
        .find(|file| file.path == "a/a.go")
        .unwrap()
        .object_id
        .clone();
    let first_b = first_files
        .iter()
        .find(|file| file.path == "b/b.go")
        .unwrap()
        .object_id
        .clone();

    fs::write(
        repository.join("a").join("a.go"),
        "package a\n\nfunc Value() int { return 3 }\n",
    )
    .unwrap();
    let incremental = engine.scan().unwrap();
    assert_eq!(incremental.languages, vec!["go"]);
    assert_eq!(incremental.changed.len(), 1);
    assert_eq!(incremental.changed[0].new, "a/a.go");
    assert_ne!(incremental.snapshot_id, full.snapshot_id);

    let (_, second_manifest) = engine.store().current().unwrap();
    let second_go = second_manifest.language("go").unwrap();
    let second_files = second_go.files.as_ref().unwrap();
    let second_a = second_files
        .iter()
        .find(|file| file.path == "a/a.go")
        .unwrap();
    let second_b = second_files
        .iter()
        .find(|file| file.path == "b/b.go")
        .unwrap();
    assert_ne!(second_a.object_id, first_a);
    assert_eq!(second_b.object_id, first_b);
}

#[test]
fn minimal_helper_response_produces_valid_native_analysis() {
    let root = TempDirectory::new("analysis");
    fs::write(
        root.path.join("go.mod"),
        "module example.com/phase2\n\ngo 1.22\n",
    )
    .unwrap();
    let helper = synthetic_helper(&root.path, r#"{"protocol_version":1,"records":[]}"#);
    let adapter = GoAdapter::with_helper(helper);
    let request = AdapterRequest {
        language: "go".into(),
        repository: root.path.clone(),
        workers: 4,
        shards: 8,
        merge_fan_in: 4,
        ..AdapterRequest::default()
    };

    let analysis = adapter.analyze(&request).unwrap();
    analysis.validate().unwrap();
    assert_eq!(analysis.header.language, "go");
    assert_eq!(analysis.header.adapter_version, "0.1.0");
    assert_eq!(analysis.header.repository, "example.com/phase2");
    assert!(!analysis.records.is_empty());
}

#[test]
fn structured_helper_diagnostics_do_not_change_fact_materialization() {
    let root = TempDirectory::new("diagnostic");
    fs::write(
        root.path.join("go.mod"),
        "module example.com/diagnostic\n\ngo 1.22\n",
    )
    .unwrap();
    let helper = synthetic_helper(
        &root.path,
        r#"{"protocol_version":1,"records":[{"record":"diagnostic","severity":"error","code":"go-package","message":"type-check failed"}]}"#,
    );
    let adapter = GoAdapter::with_helper(helper);
    let analysis = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository: root.path.clone(),
            ..AdapterRequest::default()
        })
        .unwrap();

    analysis.validate().unwrap();
    assert_eq!(analysis.header.repository, "example.com/diagnostic");
    assert!(!analysis.records.is_empty());
}

#[test]
fn helper_handshake_rejects_protocol_mismatch() {
    let root = TempDirectory::new("mismatch");
    fs::write(root.path.join("go.mod"), "module example.com/mismatch\n").unwrap();
    let helper = synthetic_helper(&root.path, r#"{"protocol_version":2,"records":[]}"#);
    let adapter = GoAdapter::with_helper(helper);
    let request = AdapterRequest {
        language: "go".into(),
        repository: root.path.clone(),
        ..AdapterRequest::default()
    };

    let error = adapter.analyze(&request).unwrap_err().to_string();
    assert!(error.contains("protocol mismatch"), "{error}");
}

#[test]
fn semantic_request_preserves_execution_plan() {
    let repository = fixture("parallel");
    let inventory = super::discovery::discover(&repository).unwrap();
    let request = AdapterRequest {
        language: "go".into(),
        repository: repository.clone(),
        workers: 3,
        shards: 6,
        merge_fan_in: 8,
        ..AdapterRequest::default()
    };
    let wire = super::semantic_request(&repository, &inventory, &request).unwrap();
    assert_eq!(wire.execution.workers, 3);
    assert_eq!(wire.execution.shards, 6);
    assert_eq!(wire.execution.merge_fan_in, 8);
}

#[test]
fn helper_invocation_is_deterministic() {
    assert_eq!(
        helper_arguments(),
        vec![
            OsString::from("--protocol-version"),
            OsString::from("1"),
            OsString::from("--helper-version"),
            OsString::from(super::protocol::HELPER_VERSION),
        ]
    );
    assert_eq!(
        helper_environment().into_iter().collect::<Vec<_>>(),
        vec![
            (OsString::from("LEXICON_HELPER"), OsString::from("go")),
            (
                OsString::from("LEXICON_HELPER_PROTOCOL"),
                OsString::from("1")
            ),
        ]
    );
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("adapters/go/testdata/oracle")
        .join(name)
}

pub(super) fn real_helper() -> HelperRunner {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    let binary = BINARY.get_or_init(|| {
        let directory =
            std::env::temp_dir().join(format!("lexicon-go-semantic-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let binary = directory.join(format!(
            "lexicon-go-semantic{}",
            std::env::consts::EXE_SUFFIX
        ));
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("adapters/go-semantic");
        let status = Command::new("go")
            .args(["build", "-o"])
            .arg(&binary)
            .arg(".")
            .current_dir(source)
            .status()
            .expect("build Go semantic helper");
        assert!(status.success(), "Go semantic helper build failed");
        binary
    });
    HelperRunner::explicit(binary.clone(), Vec::new())
}

pub(super) fn synthetic_helper(root: &Path, response: &str) -> HelperRunner {
    #[cfg(windows)]
    {
        let script = root.join("helper.ps1");
        fs::write(
            &script,
            format!(
                "$null = [Console]::In.ReadLine()\n[Console]::Out.WriteLine('{}')\n",
                response.replace('\'', "''")
            ),
        )
        .unwrap();
        let system_root = std::env::var_os("SystemRoot").unwrap();
        let program = PathBuf::from(system_root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        HelperRunner::explicit(
            program,
            vec![
                OsString::from("-NoProfile"),
                OsString::from("-File"),
                script.into_os_string(),
            ],
        )
    }
    #[cfg(not(windows))]
    {
        let script = root.join("helper.sh");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nIFS= read -r request\nprintf '%s\\n' '{}'\n",
                response.replace('\'', "'\\''")
            ),
        )
        .unwrap();
        HelperRunner::explicit(PathBuf::from("/bin/sh"), vec![script.into_os_string()])
    }
}

pub(super) struct TempDirectory {
    pub(super) path: PathBuf,
}

impl TempDirectory {
    pub(super) fn new(name: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lexicon-go-phase2-{name}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

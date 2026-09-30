use std::{
    collections::HashSet,
    ffi::OsString,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::adapters::frontend::FrontendRunner;

use super::{ClangFrontend, STRUCTURAL_FILES_PER_REQUEST, ScanInventory, clang_protocol};

#[test]
fn capabilities_uses_versioned_private_frontend_contract() {
    let root = TestDirectory::new("capabilities");
    let response = format!(
        r#"{{"protocol_version":2,"helper_version":"{}","clang_version":"clang test","capabilities":["ast","compile-database"],"compilation_database":true}}"#,
        clang_protocol::HELPER_VERSION
    );
    let frontend = ClangFrontend::with_runner(scripted_frontend(&root.path, &response));
    let capabilities = frontend.capabilities(&root.path).unwrap();

    assert_eq!(capabilities.protocol_version, 2);
    assert_eq!(capabilities.helper_version, clang_protocol::HELPER_VERSION);
    assert_eq!(capabilities.clang_version, "clang test");
    assert!(capabilities.compilation_database);
    assert_eq!(capabilities.capabilities, ["ast", "compile-database"]);
}

#[test]
fn structural_request_preserves_owned_and_context_classes() {
    let owned = HashSet::from(["src/owned.c".to_owned()]);
    let (owned_files, context_files) = super::split_ownership(
        vec!["include/context.h".into(), "src/owned.c".into()],
        &owned,
    );

    assert_eq!(owned_files, ["src/owned.c".to_owned()]);
    assert_eq!(context_files, ["include/context.h".to_owned()]);
}

#[test]
fn structural_request_v2_has_explicit_ownership_and_execution_policy() {
    let request = clang_protocol::StructuralRequest::new(
        "C:/repo".into(),
        vec!["src/main.c".into()],
        vec!["include/api.h".into()],
        4,
        16,
        4,
    );
    let value = serde_json::to_value(request).unwrap();

    assert_eq!(value["protocol_version"], 2);
    assert_eq!(value["owned_files"], serde_json::json!(["src/main.c"]));
    assert_eq!(value["context_files"], serde_json::json!(["include/api.h"]));
    assert_eq!(value["workers"], 4);
    assert_eq!(value["shards"], 16);
    assert_eq!(value["merge_fan_in"], 4);
    assert!(value.get("files").is_none());
}

#[test]
fn structural_request_normalizes_zero_execution_policy() {
    let request =
        clang_protocol::StructuralRequest::new("C:/repo".into(), Vec::new(), Vec::new(), 0, 0, 0);
    let value = serde_json::to_value(request).unwrap();

    assert_eq!(value["workers"], 1);
    assert_eq!(value["shards"], 1);
    assert_eq!(value["merge_fan_in"], 2);
}

#[test]
fn structural_uses_versioned_private_frontend_contract() {
    let root = TestDirectory::new("structural");
    fs::write(root.path.join("main.c"), b"int main(void) { return 0; }\n").unwrap();
    let response = format!(
        r#"{{"protocol_version":2,"helper_version":"{}","clang_version":"clang test","compilation_database":false,"translation_units":[{{"path":"main.c","language":"c","directory":".","arguments":["clang","-xc","main.c"],"synthesized":true}}],"files":[{{"path":"main.c","languages":["c"],"translation_units":["main.c"]}}],"diagnostics":[]}}"#,
        clang_protocol::HELPER_VERSION
    );
    let frontend = ClangFrontend::with_runner(scripted_frontend(&root.path, &response));
    let structural = frontend
        .structural(
            &root.path,
            ScanInventory {
                owned_files: vec!["main.c".into()],
                context_files: Vec::new(),
            },
            4,
            16,
            4,
        )
        .unwrap();
    let file = structural.merged_file("main.c").unwrap();

    assert_eq!(structural.file_count(), 1);
    assert_eq!(structural.translation_unit_count(), 1);
    assert_eq!(file.path, "main.c");
    assert_eq!(file.languages, ["c"]);
    assert_eq!(file.translation_units, ["main.c"]);
}

#[test]
fn structural_chunks_large_source_sets_and_merges_duplicate_observations() {
    let root = TestDirectory::new("structural-chunks");
    fs::write(root.path.join("main.c"), b"int main(void) { return 0; }\n").unwrap();
    let counter = root.path.join("calls.txt");
    let response = format!(
        r#"{{"protocol_version":2,"helper_version":"{}","clang_version":"clang test","compilation_database":false,"translation_units":[{{"path":"main.c","language":"c","directory":".","arguments":["clang","-xc","main.c"],"synthesized":true}}],"files":[{{"path":"main.c","languages":["c"],"translation_units":["main.c"]}}],"diagnostics":[]}}"#,
        clang_protocol::HELPER_VERSION
    );
    let frontend = ClangFrontend::with_runner(counting_frontend(&root.path, &counter, &response));
    let files = (0..=STRUCTURAL_FILES_PER_REQUEST)
        .map(|index| format!("file{index:03}.c"))
        .collect();

    let structural = frontend
        .structural(
            &root.path,
            ScanInventory {
                owned_files: files,
                context_files: Vec::new(),
            },
            4,
            16,
            4,
        )
        .unwrap();

    assert_eq!(fs::read_to_string(counter).unwrap().lines().count(), 2);
    assert_eq!(structural.file_count(), 1);
    assert_eq!(structural.translation_unit_count(), 1);
    assert_eq!(structural.merged_file("main.c").unwrap().path, "main.c");
}

#[test]
fn structural_sends_unobserved_headers_in_one_helper_request() {
    let root = TestDirectory::new("structural-header-batch");
    let counter = root.path.join("calls.txt");
    let response = format!(
        r#"{{"protocol_version":2,"helper_version":"{}","clang_version":"clang test","compilation_database":false,"translation_units":[],"files":[],"diagnostics":[]}}"#,
        clang_protocol::HELPER_VERSION
    );
    let frontend = ClangFrontend::with_runner(counting_frontend(&root.path, &counter, &response));

    frontend
        .structural(
            &root.path,
            ScanInventory {
                owned_files: vec![
                    "a.h".into(),
                    "include/b.hpp".into(),
                    "include/deep/c.hxx".into(),
                ],
                context_files: Vec::new(),
            },
            4,
            16,
            4,
        )
        .unwrap();

    assert_eq!(fs::read_to_string(counter).unwrap().lines().count(), 1);
}

#[test]
fn capabilities_rejects_helper_version_mismatch() {
    let root = TestDirectory::new("version-mismatch");
    let response = r#"{"protocol_version":2,"helper_version":"stale","clang_version":"clang test","capabilities":[],"compilation_database":false}"#;
    let frontend = ClangFrontend::with_runner(scripted_frontend(&root.path, response));
    let error = frontend.capabilities(&root.path).unwrap_err().to_string();

    assert!(error.contains("helper version mismatch"), "{error}");
}

fn counting_frontend(
    root: &std::path::Path,
    counter: &std::path::Path,
    response: &str,
) -> FrontendRunner {
    #[cfg(windows)]
    {
        let script = root.join("clang-counting-frontend.ps1");
        fs::write(
            &script,
            format!(
                "$null = [Console]::In.ReadLine()\nAdd-Content -LiteralPath '{}' -Value 'call'\n[Console]::Out.WriteLine('{}')\n",
                counter.display().to_string().replace('\'', "''"),
                response.replace('\'', "''")
            ),
        )
        .unwrap();
        let program = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        return FrontendRunner::explicit(
            program,
            vec![
                OsString::from("-NoProfile"),
                OsString::from("-File"),
                script.into_os_string(),
            ],
        );
    }
    #[cfg(not(windows))]
    {
        let script = root.join("clang-counting-frontend.sh");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nIFS= read -r request\nprintf 'call\\n' >> '{}'\nprintf '%s\\n' '{}'\n",
                counter.display().to_string().replace('\'', "'\\''"),
                response.replace('\'', "'\\''")
            ),
        )
        .unwrap();
        FrontendRunner::explicit(PathBuf::from("/bin/sh"), vec![script.into_os_string()])
    }
}

fn scripted_frontend(root: &std::path::Path, response: &str) -> FrontendRunner {
    #[cfg(windows)]
    {
        let script = root.join("clang-frontend.ps1");
        fs::write(
            &script,
            format!(
                "$null = [Console]::In.ReadLine()\n[Console]::Out.WriteLine('{}')\n",
                response.replace('\'', "''")
            ),
        )
        .unwrap();
        let program = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        return FrontendRunner::explicit(
            program,
            vec![
                OsString::from("-NoProfile"),
                OsString::from("-File"),
                script.into_os_string(),
            ],
        );
    }
    #[cfg(not(windows))]
    {
        let script = root.join("clang-frontend.sh");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nIFS= read -r request\nprintf '%s\\n' '{}'\n",
                response.replace('\'', "'\\''")
            ),
        )
        .unwrap();
        FrontendRunner::explicit(PathBuf::from("/bin/sh"), vec![script.into_os_string()])
    }
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lexicon-c-family-clang-{label}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

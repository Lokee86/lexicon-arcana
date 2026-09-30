use std::{
    ffi::OsString,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::adapters::frontend::FrontendRunner;

use super::{ClangFrontend, clang_protocol};

#[test]
fn capabilities_uses_versioned_private_frontend_contract() {
    let root = TestDirectory::new("capabilities");
    let response = format!(
        r#"{{"protocol_version":1,"helper_version":"{}","clang_version":"clang test","capabilities":["ast","compile-database"],"compilation_database":true}}"#,
        clang_protocol::HELPER_VERSION
    );
    let frontend = ClangFrontend::with_runner(scripted_frontend(&root.path, &response));
    let capabilities = frontend.capabilities(&root.path).unwrap();

    assert_eq!(capabilities.protocol_version, 1);
    assert_eq!(capabilities.helper_version, clang_protocol::HELPER_VERSION);
    assert_eq!(capabilities.clang_version, "clang test");
    assert!(capabilities.compilation_database);
    assert_eq!(capabilities.capabilities, ["ast", "compile-database"]);
}

#[test]
fn structural_uses_versioned_private_frontend_contract() {
    let root = TestDirectory::new("structural");
    fs::write(root.path.join("main.c"), b"int main(void) { return 0; }\n").unwrap();
    let response = format!(
        r#"{{"protocol_version":1,"helper_version":"{}","clang_version":"clang test","compilation_database":false,"translation_units":[{{"path":"main.c","language":"c","directory":".","arguments":["clang","-xc","main.c"],"synthesized":true}}],"files":[{{"path":"main.c","languages":["c"],"translation_units":["main.c"]}}],"diagnostics":[]}}"#,
        clang_protocol::HELPER_VERSION
    );
    let frontend = ClangFrontend::with_runner(scripted_frontend(&root.path, &response));
    let structural = frontend
        .structural(&root.path, vec!["main.c".into()])
        .unwrap();

    assert_eq!(structural.files.len(), 1);
    assert_eq!(structural.files[0].path, "main.c");
    assert_eq!(structural.translation_units[0].language, "c");
    assert!(structural.translation_units[0].synthesized);
}

#[test]
fn structural_chunks_large_source_sets_and_merges_duplicate_observations() {
    let root = TestDirectory::new("structural-chunks");
    fs::write(root.path.join("main.c"), b"int main(void) { return 0; }\n").unwrap();
    let counter = root.path.join("calls.txt");
    let response = format!(
        r#"{{"protocol_version":1,"helper_version":"{}","clang_version":"clang test","compilation_database":false,"translation_units":[{{"path":"main.c","language":"c","directory":".","arguments":["clang","-xc","main.c"],"synthesized":true}}],"files":[{{"path":"main.c","languages":["c"],"translation_units":["main.c"]}}],"diagnostics":[]}}"#,
        clang_protocol::HELPER_VERSION
    );
    let frontend = ClangFrontend::with_runner(counting_frontend(&root.path, &counter, &response));
    let files = (0..129).map(|index| format!("file{index:03}.c")).collect();

    let structural = frontend.structural(&root.path, files).unwrap();

    assert_eq!(fs::read_to_string(counter).unwrap().lines().count(), 2);
    assert_eq!(structural.files.len(), 1);
    assert_eq!(structural.translation_units.len(), 1);
}

#[test]
fn structural_sends_unobserved_headers_in_one_helper_request() {
    let root = TestDirectory::new("structural-header-batch");
    let counter = root.path.join("calls.txt");
    let response = format!(
        r#"{{"protocol_version":1,"helper_version":"{}","clang_version":"clang test","compilation_database":false,"translation_units":[],"files":[],"diagnostics":[]}}"#,
        clang_protocol::HELPER_VERSION
    );
    let frontend = ClangFrontend::with_runner(counting_frontend(&root.path, &counter, &response));

    frontend
        .structural(
            &root.path,
            vec![
                "a.h".into(),
                "include/b.hpp".into(),
                "include/deep/c.hxx".into(),
            ],
        )
        .unwrap();

    assert_eq!(fs::read_to_string(counter).unwrap().lines().count(), 1);
}

#[test]
fn capabilities_rejects_helper_version_mismatch() {
    let root = TestDirectory::new("version-mismatch");
    let response = r#"{"protocol_version":1,"helper_version":"stale","clang_version":"clang test","capabilities":[],"compilation_database":false}"#;
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

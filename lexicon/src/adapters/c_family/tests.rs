use std::{
    ffi::OsString,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{
    AdapterMode, AdapterRequest, FactRecord, LanguageAdapter, adapters::frontend::FrontendRunner,
};

use super::{CFamilyAdapter, clang_frontend::ClangFrontend};

const STRUCTURAL_RESPONSE: &str = r#"{"protocol_version":2,"helper_version":"0.5.0","clang_version":"clang test","compilation_database":false,"translation_units":[{"path":"main.c","language":"c","directory":".","arguments":["clang","-xc","main.c"],"synthesized":true}],"files":[{"path":"main.c","languages":["c"],"translation_units":["main.c"],"declarations":[{"compiler_id":"main","kind":"function","name":"main","qualified_name":"main","signature":"int main()","span":{"path":"main.c","start_line":1,"start_column":1,"end_line":1,"end_column":9},"callable":true,"definition":true,"internal":false,"template":false,"virtual_member":false,"function_pointer":false,"alias":false,"enum_member":false,"parameter_count":0}]}],"diagnostics":[]}"#;

#[test]
fn production_adapter_routes_through_clang_frontend() {
    let root = TestDirectory::new("production-cutover");
    fs::write(root.path.join("main.c"), b"int main(void) { return 0; }\n").unwrap();

    let frontend = ClangFrontend::with_runner(scripted_frontend(&root.path, STRUCTURAL_RESPONSE));
    let adapter = CFamilyAdapter::with_frontend(frontend);
    let analysis = adapter
        .analyze(&AdapterRequest {
            language: "c-family".into(),
            mode: AdapterMode::Full,
            repository: root.path.clone(),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(analysis.header.adapter_version, "0.6.0");
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node)
            if node.kind == "file"
                && node.path == "main.c"
                && node.attributes.as_ref().is_some_and(|attributes| {
                    attributes.get("parser").and_then(|value| value.as_str())
                        == Some("clang")
                })
    )));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node) if node.kind == "function" && node.name == "main"
    )));
}

#[test]
fn c_family_analysis_is_checkout_path_independent() {
    let first_parent = TestDirectory::new("relocation-a");
    let second_parent = TestDirectory::new("relocation-b");
    let first = first_parent.path.join("checkout");
    let second = second_parent.path.join("checkout");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    fs::write(first.join("main.c"), b"int main(void) { return 0; }\n").unwrap();
    fs::write(second.join("main.c"), b"int main(void) { return 0; }\n").unwrap();

    let analyze = |root: &std::path::Path| {
        let adapter = CFamilyAdapter::with_frontend(ClangFrontend::with_runner(scripted_frontend(
            root,
            STRUCTURAL_RESPONSE,
        )));
        adapter
            .analyze(&AdapterRequest {
                language: "c-family".into(),
                mode: AdapterMode::Full,
                repository: root.to_path_buf(),
                ..Default::default()
            })
            .unwrap()
    };

    assert_eq!(analyze(&first), analyze(&second));
}

#[test]
fn c_family_full_and_incremental_records_are_equivalent() {
    let root = TestDirectory::new("publication-equivalence");
    fs::write(root.path.join("main.c"), b"int main(void) { return 0; }\n").unwrap();
    let adapter = CFamilyAdapter::with_frontend(ClangFrontend::with_runner(scripted_frontend(
        &root.path,
        STRUCTURAL_RESPONSE,
    )));

    let full = adapter
        .analyze(&AdapterRequest {
            language: "c-family".into(),
            mode: AdapterMode::Full,
            repository: root.path.clone(),
            ..Default::default()
        })
        .unwrap();
    let incremental = adapter
        .analyze(&AdapterRequest {
            language: "c-family".into(),
            mode: AdapterMode::Incremental,
            repository: root.path.clone(),
            changed_files: vec!["main.c".into()],
            ..Default::default()
        })
        .unwrap();

    assert_eq!(full.records, incremental.records);
    assert_eq!(incremental.header.mode.as_deref(), Some("incremental"));
    assert_eq!(
        incremental.header.changed_files,
        Some(vec!["main.c".to_owned()])
    );
    assert_eq!(incremental.header.shared_complete, Some(false));
}

fn scripted_frontend(root: &std::path::Path, response: &str) -> FrontendRunner {
    #[cfg(windows)]
    {
        let script = root.join("clang-production.ps1");
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
        let script = root.join("clang-production.sh");
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
            "lexicon-c-family-{label}-{}-{}",
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

use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::{Deserialize, Serialize};

use super::{FrontendRunner, ProtocolResponse};

#[derive(Serialize)]
struct TestRequest {
    protocol_version: u32,
}

#[derive(Debug, Deserialize)]
struct TestResponse {
    protocol_version: u32,
}

impl ProtocolResponse for TestResponse {
    fn protocol_version(&self) -> u32 {
        self.protocol_version
    }
}

#[test]
fn malformed_frontend_response_is_rejected() {
    let root = TempDirectory::new("malformed");
    let runner = scripted_frontend(&root.path, "not-json", "", 0);
    let error = run(&runner, 1).unwrap_err().to_string();
    assert!(
        error.contains("decode semantic frontend response"),
        "{error}"
    );
}

#[test]
fn frontend_uses_only_the_first_response_frame() {
    let root = TempDirectory::new("first-frame");
    let runner = scripted_frontend(
        &root.path,
        "{\"protocol_version\":1}\ntrailing-output",
        "",
        0,
    );
    let response = run(&runner, 1).unwrap();
    assert_eq!(response.protocol_version, 1);
}

#[test]
fn frontend_streams_multiple_framed_payloads() {
    let root = TempDirectory::new("framed");
    let metadata = r#"{"helper_version":"test"}"#;
    let file = r#"{"path":"src/main.c"}"#;
    let response = format!(
        "{}{}",
        framed(1, "metadata", None, metadata),
        framed(1, "file", Some("src/main.c"), file)
    );
    let runner = scripted_frontend(&root.path, &response, "", 0);
    let mut seen = Vec::new();

    let metrics = runner
        .run_framed(
            Path::new("."),
            &[],
            &Default::default(),
            1,
            &TestRequest {
                protocol_version: 1,
            },
            |header, payload| {
                let mut body = String::new();
                payload
                    .read_to_string(&mut body)
                    .map_err(|error| crate::AdapterError::new(error.to_string()))?;
                seen.push((header.kind, header.path, body));
                Ok(())
            },
        )
        .unwrap();

    assert_eq!(
        seen,
        vec![
            ("metadata".into(), None, metadata.into()),
            ("file".into(), Some("src/main.c".into()), file.into()),
        ]
    );
    assert_eq!(metrics.frames, 2);
    assert_eq!(metrics.response_bytes, response.len() as u64);
    assert!(metrics.request_bytes > 0);
}

#[test]
fn frontend_protocol_mismatch_is_rejected() {
    let root = TempDirectory::new("protocol");
    let runner = scripted_frontend(&root.path, r#"{"protocol_version":2}"#, "", 0);
    let error = run(&runner, 1).unwrap_err().to_string();
    assert!(error.contains("protocol mismatch"), "{error}");
}

#[test]
fn frontend_failure_preserves_stderr() {
    let root = TempDirectory::new("failure");
    let runner = scripted_frontend(
        &root.path,
        r#"{"protocol_version":1}"#,
        "frontend exploded",
        7,
    );
    let error = run(&runner, 1).unwrap_err().to_string();
    assert!(error.contains("frontend exploded"), "{error}");
    assert!(error.contains("semantic frontend exited"), "{error}");
}

#[test]
fn missing_frontend_reports_checked_path() {
    let missing =
        std::env::temp_dir().join(format!("lexicon-missing-frontend-{}", std::process::id()));
    let runner = FrontendRunner::explicit(missing.clone(), Vec::new());
    let error = runner.resolve().unwrap_err().to_string();
    assert!(
        error.contains("semantic frontend executable not found"),
        "{error}"
    );
    assert!(error.contains(&missing.display().to_string()), "{error}");
}

fn run(
    runner: &FrontendRunner,
    expected_protocol: u32,
) -> Result<TestResponse, crate::AdapterError> {
    runner.run_json(
        Path::new("."),
        &[],
        &Default::default(),
        expected_protocol,
        &TestRequest {
            protocol_version: expected_protocol,
        },
    )
}

fn framed(protocol: u32, kind: &str, path: Option<&str>, payload: &str) -> String {
    let mut header = serde_json::json!({
        "protocol_version": protocol,
        "kind": kind,
        "bytes": payload.len(),
    });
    if let Some(path) = path {
        header["path"] = serde_json::Value::String(path.into());
    }
    format!("{}\n{}\n", serde_json::to_string(&header).unwrap(), payload)
}

fn scripted_frontend(root: &Path, stdout: &str, stderr: &str, exit_code: i32) -> FrontendRunner {
    let stdout_path = root.join("frontend-stdout.txt");
    fs::write(&stdout_path, stdout.as_bytes()).unwrap();
    #[cfg(windows)]
    {
        let script = root.join("frontend.ps1");
        fs::write(
            &script,
            format!(
                "$null = [Console]::In.ReadLine()\n[Console]::Out.Write([IO.File]::ReadAllText('{}'))\n[Console]::Error.WriteLine('{}')\nexit {}\n",
                stdout_path.display().to_string().replace("'", "''"),
                stderr.replace("'", "''"),
                exit_code
            ),
        )
        .unwrap();
        let system_root = std::env::var_os("SystemRoot").unwrap();
        let program = PathBuf::from(system_root)
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
        let script = root.join("frontend.sh");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nIFS= read -r request\ncat '{}'\nprintf '%s\\n' '{}' >&2\nexit {}\n",
                stdout_path.display().to_string().replace("'", "'\\''"),
                stderr.replace("'", "'\\''"),
                exit_code
            ),
        )
        .unwrap();
        FrontendRunner::explicit(PathBuf::from("/bin/sh"), vec![script.into_os_string()])
    }
}

struct TempDirectory {
    path: PathBuf,
}

impl TempDirectory {
    fn new(label: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lexicon-frontend-{label}-{}-{}",
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

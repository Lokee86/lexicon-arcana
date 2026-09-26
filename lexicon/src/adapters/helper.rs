use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Instant,
};

use serde::{Serialize, de::DeserializeOwned};

use super::{
    AdapterError,
    helper_capture::{capture_stderr, replay_stderr, stderr_suffix, terminate},
};

const MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct HelperRunner {
    candidates: Vec<PathBuf>,
    prefix_args: Vec<OsString>,
}

impl HelperRunner {
    pub(crate) fn discover(
        adapter_root: &Path,
        directory: &str,
        executable: &str,
        environment: &str,
    ) -> Self {
        let executable = executable_name(executable);
        let mut candidates = Vec::new();
        if let Some(value) = std::env::var_os(environment) {
            candidates.push(PathBuf::from(value));
        }
        candidates.push(adapter_root.join(directory).join(&executable));
        candidates.push(adapter_root.join(&executable));
        if let Ok(current) = std::env::current_exe()
            && let Some(parent) = current.parent()
        {
            candidates.push(parent.join(&executable));
        }
        Self {
            candidates,
            prefix_args: Vec::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn explicit(program: PathBuf, prefix_args: Vec<OsString>) -> Self {
        Self {
            candidates: vec![program],
            prefix_args,
        }
    }

    pub(crate) fn run_json<Request, Response>(
        &self,
        current_dir: &Path,
        arguments: &[OsString],
        environment: &BTreeMap<OsString, OsString>,
        expected_protocol: u32,
        request: &Request,
    ) -> Result<Response, AdapterError>
    where
        Request: Serialize,
        Response: DeserializeOwned,
    {
        let request = serde_json::to_vec(request)
            .map_err(|error| AdapterError::new(format!("encode helper request: {error}")))?;
        let request_bytes = request.len() as u64;
        let profile_go = crate::perf::enabled()
            && environment.iter().any(|(key, value)| {
                key.to_string_lossy() == "LEXICON_HELPER" && value.to_string_lossy() == "go"
            });
        let ipc_started = profile_go.then(Instant::now);
        let program = self.resolve()?;
        let mut child = Command::new(&program)
            .args(&self.prefix_args)
            .args(arguments)
            .envs(environment)
            .current_dir(current_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                AdapterError::new(format!(
                    "start semantic helper {}: {error}",
                    program.display()
                ))
            })?;
        let stderr_thread = capture_stderr(child.stderr.take().expect("stderr was piped"));

        let mut stdin = child.stdin.take().expect("stdin was piped");
        let write_result = stdin
            .write_all(&request)
            .and_then(|_| stdin.write_all(b"\n"));
        drop(stdin);
        if let Err(error) = write_result {
            return Err(terminate(
                &mut child,
                stderr_thread,
                format!("write semantic helper request: {error}"),
            ));
        }

        let stdout = child.stdout.take().expect("stdout was piped");
        let mut reader = BufReader::new(stdout);
        let mut frame = Vec::new();
        let read_result = reader
            .by_ref()
            .take(MAX_RESPONSE_BYTES + 1)
            .read_until(b'\n', &mut frame);
        if let Err(error) = read_result {
            return Err(terminate(
                &mut child,
                stderr_thread,
                format!("read semantic helper response: {error}"),
            ));
        }
        if frame.is_empty() || frame.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(terminate(
                &mut child,
                stderr_thread,
                "semantic helper response frame is empty or too large".into(),
            ));
        }
        while matches!(frame.last(), Some(b'\n' | b'\r')) {
            frame.pop();
        }

        let response_bytes = frame.len() as u64;
        let decode_started = profile_go.then(Instant::now);
        let value: serde_json::Value = match serde_json::from_slice(&frame) {
            Ok(value) => value,
            Err(error) => {
                return Err(terminate(
                    &mut child,
                    stderr_thread,
                    format!("decode semantic helper response: {error}"),
                ));
            }
        };
        let version = value
            .get("protocol_version")
            .and_then(serde_json::Value::as_u64);
        if version != Some(u64::from(expected_protocol)) {
            return Err(terminate(
                &mut child,
                stderr_thread,
                format!(
                    "semantic helper protocol mismatch: got {version:?}, expected {expected_protocol}"
                ),
            ));
        }
        let response = match serde_json::from_value(value) {
            Ok(response) => response,
            Err(error) => {
                return Err(terminate(
                    &mut child,
                    stderr_thread,
                    format!("decode semantic helper response: {error}"),
                ));
            }
        };
        let decode_elapsed = decode_started.map(|started| started.elapsed());

        drop(reader);
        let status = child
            .wait()
            .map_err(|error| AdapterError::new(format!("wait for semantic helper: {error}")))?;
        let stderr = stderr_thread.join().unwrap_or_default();
        if !status.success() {
            return Err(AdapterError::new(format!(
                "semantic helper exited with {status}{}",
                stderr_suffix(&stderr)
            )));
        }
        if let (Some(ipc_started), Some(decode_elapsed)) = (ipc_started, decode_elapsed) {
            replay_stderr(&stderr);
            crate::perf::emit(
                "go.helper_ipc",
                ipc_started.elapsed(),
                &[
                    ("helper_request_bytes", request_bytes),
                    ("helper_response_bytes", response_bytes),
                ],
            );
            crate::perf::emit(
                "go.rust_response_decode",
                decode_elapsed,
                &[("helper_response_bytes", response_bytes)],
            );
        }
        Ok(response)
    }

    fn resolve(&self) -> Result<PathBuf, AdapterError> {
        self.candidates
            .iter()
            .find(|candidate| candidate.is_file())
            .cloned()
            .ok_or_else(|| {
                AdapterError::new(format!(
                    "semantic helper executable not found; checked {}",
                    self.candidates
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })
    }
}

fn executable_name(name: &str) -> OsString {
    let mut value = OsString::from(name);
    value.push(std::env::consts::EXE_SUFFIX);
    value
}

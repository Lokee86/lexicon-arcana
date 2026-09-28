use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Instant,
};

use serde::{Serialize, de::DeserializeOwned};

use crate::adapters::AdapterError;

use super::capture::{capture_stderr, replay_stderr, stderr_suffix, terminate};

// Keep frontend IPC bounded while allowing measured multi-module repository responses.
const MAX_RESPONSE_BYTES: u64 = 128 * 1024 * 1024;

pub(crate) trait ProtocolResponse {
    fn protocol_version(&self) -> u32;
}

#[derive(Debug, Clone)]
pub(crate) struct FrontendRunner {
    candidates: Vec<PathBuf>,
    prefix_args: Vec<OsString>,
    environment_override: Option<String>,
    metric_namespace: Option<String>,
}

impl FrontendRunner {
    pub(crate) fn discover(
        adapter_root: &Path,
        directory: &str,
        executable: &str,
        environment: &str,
        metric_namespace: &str,
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
            environment_override: Some(environment.to_owned()),
            metric_namespace: Some(metric_namespace.to_owned()),
        }
    }

    #[cfg(test)]
    pub(crate) fn explicit(program: PathBuf, prefix_args: Vec<OsString>) -> Self {
        Self {
            candidates: vec![program],
            prefix_args,
            environment_override: None,
            metric_namespace: None,
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
        Response: DeserializeOwned + ProtocolResponse,
    {
        let request = serde_json::to_vec(request)
            .map_err(|error| AdapterError::new(format!("encode frontend request: {error}")))?;
        let request_bytes = request.len() as u64;
        let profile = crate::perf::enabled() && self.metric_namespace.is_some();
        let ipc_started = profile.then(Instant::now);
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
                    "start semantic frontend {}: {error}",
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
                format!("write semantic frontend request: {error}"),
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
                format!("read semantic frontend response: {error}"),
            ));
        }
        if frame.is_empty() || frame.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(terminate(
                &mut child,
                stderr_thread,
                "semantic frontend response frame is empty or too large".into(),
            ));
        }
        while matches!(frame.last(), Some(b'\n' | b'\r')) {
            frame.pop();
        }

        let response_bytes = frame.len() as u64;
        let decode_started = profile.then(Instant::now);
        let response: Response = match serde_json::from_slice(&frame) {
            Ok(response) => response,
            Err(error) => {
                return Err(terminate(
                    &mut child,
                    stderr_thread,
                    format!("decode semantic frontend response: {error}"),
                ));
            }
        };
        let version = response.protocol_version();
        if version != expected_protocol {
            return Err(terminate(
                &mut child,
                stderr_thread,
                format!(
                    "semantic frontend protocol mismatch: got {version}, expected {expected_protocol}"
                ),
            ));
        }
        let decode_elapsed = decode_started.map(|started| started.elapsed());

        drop(reader);
        let status = child
            .wait()
            .map_err(|error| AdapterError::new(format!("wait for semantic frontend: {error}")))?;
        let stderr = stderr_thread.join().unwrap_or_default();
        if !status.success() {
            return Err(AdapterError::new(format!(
                "semantic frontend exited with {status}{}",
                stderr_suffix(&stderr)
            )));
        }
        if let (Some(namespace), Some(ipc_started), Some(decode_elapsed)) = (
            self.metric_namespace.as_deref(),
            ipc_started,
            decode_elapsed,
        ) {
            replay_stderr(&stderr);
            let ipc_stage = format!("{namespace}.helper_ipc");
            crate::perf::emit(
                &ipc_stage,
                ipc_started.elapsed(),
                &[
                    ("helper_request_bytes", request_bytes),
                    ("helper_response_bytes", response_bytes),
                ],
            );
            let decode_stage = format!("{namespace}.rust_response_decode");
            crate::perf::emit(
                &decode_stage,
                decode_elapsed,
                &[("helper_response_bytes", response_bytes)],
            );
        }
        Ok(response)
    }

    pub(crate) fn resolve(&self) -> Result<PathBuf, AdapterError> {
        self.candidates
            .iter()
            .find(|candidate| candidate.is_file())
            .cloned()
            .ok_or_else(|| {
                let checked = self
                    .candidates
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let override_hint = self
                    .environment_override
                    .as_deref()
                    .map(|name| format!("; set {name} to an explicit frontend path"))
                    .unwrap_or_default();
                AdapterError::new(format!(
                    "semantic frontend executable not found; install the packaged frontend{override_hint}; checked {checked}"
                ))
            })
    }
}

fn executable_name(name: &str) -> OsString {
    let mut value = OsString::from(name);
    value.push(std::env::consts::EXE_SUFFIX);
    value
}

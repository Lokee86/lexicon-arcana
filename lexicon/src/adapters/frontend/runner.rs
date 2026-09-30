use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{File, OpenOptions, remove_file},
    io::{BufRead, BufReader, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

use serde::{Serialize, de::DeserializeOwned};

use crate::adapters::AdapterError;

use super::capture::{capture_stderr, replay_stderr, stderr_suffix, terminate};

// Keep frontend IPC bounded while allowing measured whole-repository compiler responses.
// Large compiler frontends are spooled to disk so helper memory is released before decode.
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024 * 1024;
static RESPONSE_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
        let mut response_frame = ResponseFrameFile::create()?;
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
        let response_bytes = match spool_response_frame(&mut reader, &mut response_frame.file) {
            Ok(bytes) => bytes,
            Err(error) => {
                drop(reader);
                return Err(terminate(
                    &mut child,
                    stderr_thread,
                    format!("read semantic frontend response: {error}"),
                ));
            }
        };
        if response_bytes == 0 || response_bytes > MAX_RESPONSE_BYTES {
            drop(reader);
            return Err(terminate(
                &mut child,
                stderr_thread,
                format!(
                    "semantic frontend response frame is empty or too large: read {response_bytes} bytes with a {MAX_RESPONSE_BYTES}-byte limit"
                ),
            ));
        }

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

        response_frame
            .file
            .seek(SeekFrom::Start(0))
            .map_err(|error| {
                AdapterError::new(format!("rewind semantic frontend response: {error}"))
            })?;
        let decode_started = profile.then(Instant::now);
        let response: Response =
            serde_json::from_reader(&mut response_frame.file).map_err(|error| {
                AdapterError::new(format!(
                    "decode semantic frontend response: {error}{}",
                    stderr_suffix(&stderr)
                ))
            })?;
        let version = response.protocol_version();
        if version != expected_protocol {
            return Err(AdapterError::new(format!(
                "semantic frontend protocol mismatch: got {version}, expected {expected_protocol}{}",
                stderr_suffix(&stderr)
            )));
        }
        let decode_elapsed = decode_started.map(|started| started.elapsed());
        if let (Some(namespace), Some(ipc_started), Some(decode_elapsed)) = (
            self.metric_namespace.as_deref(),
            ipc_started,
            decode_elapsed,
        ) {
            replay_stderr(&stderr);
            let ipc_stage = format!("{namespace}.helper.ipc");
            crate::perf::emit(
                &ipc_stage,
                ipc_started.elapsed(),
                &[
                    ("helper_request_bytes", request_bytes),
                    ("helper_response_bytes", response_bytes),
                ],
            );
            let decode_stage = format!("{namespace}.frontend_response_decode");
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

fn spool_response_frame<R: BufRead>(reader: &mut R, output: &mut File) -> std::io::Result<u64> {
    let mut written = 0_u64;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            break;
        }

        let newline = available.iter().position(|byte| *byte == b'\n');
        let frame_bytes = newline.map_or(available.len(), |index| index + 1);
        let remaining = (MAX_RESPONSE_BYTES + 1).saturating_sub(written) as usize;
        let copy_bytes = frame_bytes.min(remaining);
        output.write_all(&available[..copy_bytes])?;
        reader.consume(copy_bytes);
        written += copy_bytes as u64;

        if newline.is_some() || copy_bytes < frame_bytes || written > MAX_RESPONSE_BYTES {
            break;
        }
    }
    Ok(written)
}

struct ResponseFrameFile {
    file: File,
    path: PathBuf,
}

impl ResponseFrameFile {
    fn create() -> Result<Self, AdapterError> {
        loop {
            let sequence = RESPONSE_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "lexicon-frontend-response-{}-{sequence}.json",
                std::process::id()
            ));
            match OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => return Ok(Self { file, path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(AdapterError::new(format!(
                        "create semantic frontend response spool: {error}"
                    )));
                }
            }
        }
    }
}

impl Drop for ResponseFrameFile {
    fn drop(&mut self) {
        let _ = remove_file(&self.path);
    }
}

fn executable_name(name: &str) -> OsString {
    let mut value = OsString::from(name);
    value.push(std::env::consts::EXE_SUFFIX);
    value
}

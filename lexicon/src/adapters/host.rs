use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use crate::languages::supports_streaming_output;

use super::runtime::prepare_runtime;
use super::{AdapterError, AdapterRequest, NativeAdapter, command_spec};

pub struct AdapterHost {
    root: PathBuf,
    native: BTreeMap<String, Arc<dyn NativeAdapter>>,
}

impl AdapterHost {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            native: BTreeMap::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn register_native(
        &mut self,
        language: impl Into<String>,
        adapter: Arc<dyn NativeAdapter>,
    ) {
        self.native.insert(language.into(), adapter);
    }

    pub fn run(&self, request: &AdapterRequest) -> Result<(), AdapterError> {
        if let Some(adapter) = self.native.get(&request.language) {
            if let Some(parent) = request.output.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut output = fs::File::create(&request.output)?;
            adapter.run(request, &mut output)?;
            ensure_output(request)
        } else {
            self.run_process(request)
        }
    }

    pub fn run_stream<T>(
        &self,
        request: &AdapterRequest,
        consume: impl FnOnce(&mut dyn Read) -> Result<T, AdapterError>,
    ) -> Result<T, AdapterError> {
        if let Some(adapter) = self.native.get(&request.language) {
            let mut bytes = Vec::new();
            adapter.run(request, &mut bytes)?;
            return consume(&mut Cursor::new(bytes));
        }
        if !supports_streaming_output(&request.language) {
            self.run_process(request)?;
            let mut file = fs::File::open(&request.output).map_err(|error| {
                AdapterError::new(format!("open {} adapter output: {error}", request.language))
            })?;
            return consume(&mut file);
        }
        self.run_process_stream(request, consume)
    }

    fn run_process(&self, request: &AdapterRequest) -> Result<(), AdapterError> {
        if let Some(parent) = request.output.parent() {
            fs::create_dir_all(parent)?;
        }
        prepare_runtime(&self.root, request)?;
        let spec = command_spec(&self.root, request)?;
        let output = spec.build().output().map_err(|error| {
            AdapterError::new(format!("start {} adapter: {error}", request.language))
        })?;
        if !output.status.success() {
            return Err(process_failure(request, output.status, &output.stderr));
        }
        ensure_output(request)
    }

    fn run_process_stream<T>(
        &self,
        request: &AdapterRequest,
        consume: impl FnOnce(&mut dyn Read) -> Result<T, AdapterError>,
    ) -> Result<T, AdapterError> {
        prepare_runtime(&self.root, request)?;
        let mut streaming = request.clone();
        streaming.output = PathBuf::from("-");
        let spec = command_spec(&self.root, &streaming)?;
        let mut command = spec.build();
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|error| {
            AdapterError::new(format!("start {} adapter: {error}", request.language))
        })?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| AdapterError::new("adapter stdout was not captured"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| AdapterError::new("adapter stderr was not captured"))?;
        let stderr_reader = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stderr.read_to_end(&mut bytes);
            bytes
        });

        let consumed = consume(&mut stdout);
        if consumed.is_err() {
            let _ = child.kill();
        }
        let status = child.wait().map_err(AdapterError::from)?;
        let stderr = stderr_reader
            .join()
            .map_err(|_| AdapterError::new("adapter stderr reader panicked"))?;

        let value = consumed.map_err(|error| {
            AdapterError::new(format!("read {} adapter output: {error}", request.language))
        })?;
        if !status.success() {
            return Err(process_failure(request, status, &stderr));
        }
        Ok(value)
    }
}

fn ensure_output(request: &AdapterRequest) -> Result<(), AdapterError> {
    match fs::metadata(&request.output) {
        Ok(metadata) if metadata.len() > 0 => Ok(()),
        _ => Err(AdapterError::new(format!(
            "{} adapter produced no output",
            request.language
        ))),
    }
}

fn process_failure(
    request: &AdapterRequest,
    status: std::process::ExitStatus,
    stderr: &[u8],
) -> AdapterError {
    AdapterError::new(format!(
        "{} adapter failed: {}\n{}",
        request.language,
        status,
        String::from_utf8_lossy(stderr).trim()
    ))
}

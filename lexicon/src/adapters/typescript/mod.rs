mod fingerprint;
#[cfg(test)]
mod tests;

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{AdapterError, AdapterMode, AdapterRequest, Analysis, LanguageAdapter};

pub const ADAPTER_VERSION: &str = "0.1.0";

const NODE_ENVIRONMENT: &str = "LEXICON_TYPESCRIPT_NODE";
const ENTRYPOINT_ENVIRONMENT: &str = "LEXICON_TYPESCRIPT_ADAPTER";

#[derive(Debug, Clone)]
pub struct TypeScriptAdapter {
    node: OsString,
    entrypoint: PathBuf,
}

impl TypeScriptAdapter {
    pub(crate) fn new(adapter_root: &Path) -> Self {
        let node = std::env::var_os(NODE_ENVIRONMENT).unwrap_or_else(|| OsString::from("node"));
        let entrypoint = std::env::var_os(ENTRYPOINT_ENVIRONMENT)
            .map(PathBuf::from)
            .unwrap_or_else(|| adapter_root.join("typescript").join("dist").join("cli.js"));
        Self { node, entrypoint }
    }

    fn verify_entrypoint(&self) -> Result<(), AdapterError> {
        if !self.entrypoint.is_file() {
            return Err(AdapterError::new(format!(
                "TypeScript adapter entrypoint not found: {}; build/install the packaged adapter or set {ENTRYPOINT_ENVIRONMENT}",
                self.entrypoint.display()
            )));
        }
        Ok(())
    }

    fn verify_runtime(&self) -> Result<(), AdapterError> {
        self.verify_entrypoint()?;
        let output = Command::new(&self.node)
            .arg("--version")
            .output()
            .map_err(|error| {
                AdapterError::new(format!(
                    "TypeScript runtime executable {:?} is unavailable: {error}; install Node.js 22 or set {NODE_ENVIRONMENT}",
                    self.node
                ))
            })?;
        if !output.status.success() {
            return Err(AdapterError::new(format!(
                "TypeScript runtime executable {:?} failed version check with {}{}",
                self.node,
                output.status,
                stderr_suffix(&output.stderr)
            )));
        }
        Ok(())
    }

    fn command_arguments(&self, request: &AdapterRequest) -> Vec<OsString> {
        let mut arguments = vec![
            self.entrypoint.as_os_str().to_owned(),
            OsString::from("--repo"),
            request.repository.as_os_str().to_owned(),
            OsString::from("--output"),
            OsString::from("-"),
        ];
        if request.mode == AdapterMode::Incremental {
            for path in &request.changed_files {
                arguments.push(OsString::from("--changed-file"));
                arguments.push(OsString::from(normalize_path(path)));
            }
            for path in &request.removed_files {
                arguments.push(OsString::from("--removed-file"));
                arguments.push(OsString::from(normalize_path(path)));
            }
        }
        arguments
    }
}

pub(crate) fn verify_runtime_helper(adapter_root: &Path) -> Result<(), AdapterError> {
    TypeScriptAdapter::new(adapter_root).verify_runtime()
}

impl LanguageAdapter for TypeScriptAdapter {
    fn implementation_version(&self) -> &'static str {
        ADAPTER_VERSION
    }

    fn implementation_fingerprint(&self) -> String {
        fingerprint::implementation_fingerprint(ADAPTER_VERSION)
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        if request.language != "typescript" {
            return Err(AdapterError::new(
                "TypeScript adapter received another language",
            ));
        }
        self.verify_entrypoint()?;
        let output = Command::new(&self.node)
            .args(self.command_arguments(request))
            .current_dir(&request.repository)
            .output()
            .map_err(|error| {
                AdapterError::new(format!(
                    "start TypeScript adapter with {:?}: {error}",
                    self.node
                ))
            })?;
        if !output.status.success() {
            return Err(AdapterError::new(format!(
                "TypeScript adapter exited with {}{}",
                output.status,
                stderr_suffix(&output.stderr)
            )));
        }
        let stdout = String::from_utf8(output.stdout).map_err(|error| {
            AdapterError::new(format!("TypeScript adapter output is not UTF-8: {error}"))
        })?;
        decode_output(&stdout)
    }
}

fn decode_output(stdout: &str) -> Result<Analysis, AdapterError> {
    Analysis::parse_unvalidated(stdout)
        .map_err(|error| AdapterError::new(format!("decode TypeScript adapter output: {error}")))
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn stderr_suffix(stderr: &[u8]) -> String {
    if stderr.is_empty() {
        return String::new();
    }
    const LIMIT: usize = 16 * 1024;
    let start = stderr.len().saturating_sub(LIMIT);
    let text = String::from_utf8_lossy(&stderr[start..]);
    let trimmed = text.trim();
    (!trimmed.is_empty())
        .then(|| format!(": {trimmed}"))
        .unwrap_or_default()
}

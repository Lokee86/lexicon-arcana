use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

use crate::{AdapterError, adapters::frontend::FrontendRunner};

use super::clang_protocol::{self, StructuralRequest, StructuralResponse};
#[cfg(test)]
use super::clang_protocol::{CapabilitiesRequest, CapabilitiesResponse};

#[cfg(test)]
mod tests;

const HELPER_DIRECTORY: &str = "c-family-clang";
const HELPER_EXECUTABLE: &str = "lexicon-c-family-clang";
const HELPER_ENVIRONMENT: &str = "LEXICON_C_FAMILY_CLANG_HELPER";

#[derive(Debug, Clone)]
pub(crate) struct ClangFrontend {
    runner: FrontendRunner,
}

impl ClangFrontend {
    pub(crate) fn discover(adapter_root: &Path) -> Self {
        Self {
            runner: FrontendRunner::discover(
                adapter_root,
                HELPER_DIRECTORY,
                HELPER_EXECUTABLE,
                HELPER_ENVIRONMENT,
                "c-family.clang",
            ),
        }
    }

    pub(crate) fn resolve(&self) -> Result<PathBuf, AdapterError> {
        self.runner.resolve()
    }

    #[cfg(test)]
    pub(crate) fn capabilities(
        &self,
        repository: &Path,
    ) -> Result<CapabilitiesResponse, AdapterError> {
        let repository = repository.canonicalize().map_err(|error| {
            AdapterError::new(format!(
                "cannot resolve C-family repository {}: {error}",
                repository.display()
            ))
        })?;
        if !repository.is_dir() {
            return Err(AdapterError::new(
                "C-family repository path is not a directory",
            ));
        }
        let repository_root = repository
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| AdapterError::new("repository path is not valid UTF-8"))?;
        let response: CapabilitiesResponse = self.runner.run_json(
            &repository,
            &helper_arguments(),
            &helper_environment(),
            clang_protocol::PROTOCOL_VERSION,
            &CapabilitiesRequest::new(repository_root),
        )?;
        verify_helper_version(&response.helper_version)?;
        Ok(response)
    }

    pub(crate) fn structural(
        &self,
        repository: &Path,
        files: Vec<String>,
    ) -> Result<StructuralResponse, AdapterError> {
        let repository = repository.canonicalize().map_err(|error| {
            AdapterError::new(format!(
                "cannot resolve C-family repository {}: {error}",
                repository.display()
            ))
        })?;
        if !repository.is_dir() {
            return Err(AdapterError::new(
                "C-family repository path is not a directory",
            ));
        }
        let repository_root = repository
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| AdapterError::new("repository path is not valid UTF-8"))?;
        let response: StructuralResponse = self.runner.run_json(
            &repository,
            &helper_arguments(),
            &helper_environment(),
            clang_protocol::PROTOCOL_VERSION,
            &StructuralRequest::new(repository_root, files),
        )?;
        verify_helper_version(&response.helper_version)?;
        Ok(response)
    }

    #[cfg(test)]
    pub(crate) fn with_runner(runner: FrontendRunner) -> Self {
        Self { runner }
    }
}

fn verify_helper_version(actual: &str) -> Result<(), AdapterError> {
    if actual == clang_protocol::HELPER_VERSION {
        return Ok(());
    }
    Err(AdapterError::new(format!(
        "C-family Clang helper version mismatch: got {actual:?}, expected {:?}",
        clang_protocol::HELPER_VERSION
    )))
}

fn helper_arguments() -> Vec<OsString> {
    vec![
        OsString::from("--protocol-version"),
        OsString::from(clang_protocol::PROTOCOL_VERSION.to_string()),
        OsString::from("--helper-version"),
        OsString::from(clang_protocol::HELPER_VERSION),
    ]
}

fn helper_environment() -> BTreeMap<OsString, OsString> {
    BTreeMap::from([
        (
            OsString::from("LEXICON_HELPER"),
            OsString::from("c-family-clang"),
        ),
        (
            OsString::from("LEXICON_HELPER_PROTOCOL"),
            OsString::from(clang_protocol::PROTOCOL_VERSION.to_string()),
        ),
    ])
}

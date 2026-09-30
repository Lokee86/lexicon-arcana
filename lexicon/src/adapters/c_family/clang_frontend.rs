use std::{
    collections::{BTreeMap, HashSet},
    ffi::OsString,
    fmt::Debug,
    path::{Path, PathBuf},
};

use crate::{AdapterError, adapters::frontend::FrontendRunner};

use super::clang_protocol::{self, FileObservation, StructuralRequest, StructuralResponse};
#[cfg(test)]
use super::clang_protocol::{CapabilitiesRequest, CapabilitiesResponse};

#[cfg(test)]
mod tests;

const HELPER_DIRECTORY: &str = "c-family-clang";
const HELPER_EXECUTABLE: &str = "lexicon-c-family-clang";
const HELPER_ENVIRONMENT: &str = "LEXICON_C_FAMILY_CLANG_HELPER";
const STRUCTURAL_FILES_PER_REQUEST: usize = 128;

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
        let mut files = files;
        files.sort();
        files.dedup();

        if files.is_empty() {
            return self.run_structural_request(&repository, repository_root, files);
        }

        let (sources, headers): (Vec<_>, Vec<_>) = files
            .into_iter()
            .partition(|path| !super::discovery::is_header_path(path));

        let mut response: Option<StructuralResponse> = None;
        for chunk in sources.chunks(STRUCTURAL_FILES_PER_REQUEST) {
            merge_response(
                &mut response,
                self.run_structural_request(&repository, repository_root.clone(), chunk.to_vec())?,
            )?;
        }

        let observed = response
            .as_ref()
            .map(|value| {
                value
                    .files
                    .iter()
                    .map(|file| file.path.clone())
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        let orphan_headers = headers
            .into_iter()
            .filter(|path| !observed.contains(path.as_str()))
            .collect::<Vec<_>>();
        if !orphan_headers.is_empty() {
            merge_response(
                &mut response,
                self.run_structural_request(&repository, repository_root.clone(), orphan_headers)?,
            )?;
        }

        response.ok_or_else(|| AdapterError::new("C-family Clang frontend returned no response"))
    }

    fn run_structural_request(
        &self,
        repository: &Path,
        repository_root: String,
        files: Vec<String>,
    ) -> Result<StructuralResponse, AdapterError> {
        let response: StructuralResponse = self.runner.run_json(
            repository,
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

fn merge_response(
    target: &mut Option<StructuralResponse>,
    mut incoming: StructuralResponse,
) -> Result<(), AdapterError> {
    let Some(current) = target.as_mut() else {
        *target = Some(incoming);
        return Ok(());
    };
    if current.protocol_version != incoming.protocol_version
        || current.helper_version != incoming.helper_version
        || current.clang_version != incoming.clang_version
        || current.compilation_database != incoming.compilation_database
    {
        return Err(AdapterError::new(
            "inconsistent C-family Clang metadata across structural response chunks",
        ));
    }

    current
        .translation_units
        .append(&mut incoming.translation_units);
    dedup_exact(&mut current.translation_units);
    current.diagnostics.append(&mut incoming.diagnostics);
    dedup_exact(&mut current.diagnostics);

    let mut indexes = current
        .files
        .iter()
        .enumerate()
        .map(|(index, file)| (file.path.clone(), index))
        .collect::<BTreeMap<_, _>>();
    for file in incoming.files {
        if let Some(index) = indexes.get(&file.path).copied() {
            merge_file(&mut current.files[index], file);
        } else {
            indexes.insert(file.path.clone(), current.files.len());
            current.files.push(file);
        }
    }
    current
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    Ok(())
}

fn dedup_exact<T: Debug>(values: &mut Vec<T>) {
    let mut seen = HashSet::with_capacity(values.len());
    values.retain(|value| seen.insert(format!("{value:?}")));
}

fn merge_file(target: &mut FileObservation, mut incoming: FileObservation) {
    target.languages.append(&mut incoming.languages);
    target.languages.sort();
    target.languages.dedup();
    target
        .translation_units
        .append(&mut incoming.translation_units);
    target.translation_units.sort();
    target.translation_units.dedup();

    target.declarations.append(&mut incoming.declarations);
    dedup_exact(&mut target.declarations);
    target.includes.append(&mut incoming.includes);
    dedup_exact(&mut target.includes);
    target.macros.append(&mut incoming.macros);
    dedup_exact(&mut target.macros);
    target.relationships.append(&mut incoming.relationships);
    dedup_exact(&mut target.relationships);
    target.calls.append(&mut incoming.calls);
    dedup_exact(&mut target.calls);
    target
        .pointer_bindings
        .append(&mut incoming.pointer_bindings);
    dedup_exact(&mut target.pointer_bindings);
    target.accesses.append(&mut incoming.accesses);
    dedup_exact(&mut target.accesses);
    target.diagnostics.append(&mut incoming.diagnostics);
    dedup_exact(&mut target.diagnostics);
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

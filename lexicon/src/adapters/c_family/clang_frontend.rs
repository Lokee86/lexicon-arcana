use std::{
    collections::{BTreeMap, HashSet},
    ffi::OsString,
    fmt::Debug,
    fs::{self, File, OpenOptions},
    io::{BufReader, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{AdapterError, adapters::frontend::FrontendRunner};

#[cfg(test)]
use super::clang_protocol::{CapabilitiesRequest, CapabilitiesResponse};
use super::{
    clang_protocol::{
        self, ContextIdentityObservation, FileObservation, StructuralRequest, StructuralResponse,
    },
    inventory::ScanInventory,
};

#[cfg(test)]
mod tests;

const HELPER_DIRECTORY: &str = "c-family-clang";
const HELPER_EXECUTABLE: &str = "lexicon-c-family-clang";
const HELPER_ENVIRONMENT: &str = "LEXICON_C_FAMILY_CLANG_HELPER";
static OBSERVATION_STORE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
struct StructuralMetadata {
    protocol_version: u32,
    helper_version: String,
    clang_version: String,
    compilation_database: bool,
}

#[derive(Debug)]
pub(crate) struct StructuralObservationStore {
    root: PathBuf,
    fragments: BTreeMap<String, PathBuf>,
    translation_units: HashSet<String>,
    context_identities: Vec<ContextIdentityObservation>,
    metadata: Option<StructuralMetadata>,
}

impl StructuralObservationStore {
    fn create() -> Result<Self, AdapterError> {
        loop {
            let sequence = OBSERVATION_STORE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "lexicon-c-family-observations-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&root) {
                Ok(()) => {
                    return Ok(Self {
                        root,
                        fragments: BTreeMap::new(),
                        translation_units: HashSet::new(),
                        context_identities: Vec::new(),
                        metadata: None,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(AdapterError::new(format!(
                        "create C-family observation spool: {error}"
                    )));
                }
            }
        }
    }

    fn append_response(&mut self, response: StructuralResponse) -> Result<(), AdapterError> {
        let metadata = StructuralMetadata {
            protocol_version: response.protocol_version,
            helper_version: response.helper_version,
            clang_version: response.clang_version,
            compilation_database: response.compilation_database,
        };
        if let Some(current) = &self.metadata {
            if current != &metadata {
                return Err(AdapterError::new(
                    "inconsistent C-family Clang metadata across structural response chunks",
                ));
            }
        } else {
            self.metadata = Some(metadata);
        }

        for translation_unit in response.translation_units {
            self.translation_units
                .insert(format!("{translation_unit:?}"));
        }

        self.context_identities.extend(response.context_identities);

        for file in response.files {
            let observation_path = file.path.clone();
            let fragment_path = if let Some(path) = self.fragments.get(&observation_path) {
                path.clone()
            } else {
                let path = self
                    .root
                    .join(format!("file-{:06}.jsonl", self.fragments.len()));
                self.fragments
                    .insert(observation_path.clone(), path.clone());
                path
            };
            let mut output = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&fragment_path)
                .map_err(|error| {
                    AdapterError::new(format!(
                        "open C-family observation spool {}: {error}",
                        fragment_path.display()
                    ))
                })?;
            serde_json::to_writer(&mut output, &file).map_err(|error| {
                AdapterError::new(format!(
                    "write C-family observation spool {}: {error}",
                    fragment_path.display()
                ))
            })?;
            output.write_all(b"\n").map_err(|error| {
                AdapterError::new(format!(
                    "finish C-family observation spool {}: {error}",
                    fragment_path.display()
                ))
            })?;
        }
        Ok(())
    }

    pub(crate) fn file_count(&self) -> usize {
        self.fragments.len()
    }

    pub(crate) fn translation_unit_count(&self) -> usize {
        self.translation_units.len()
    }

    pub(crate) fn file_paths(&self) -> Vec<String> {
        self.fragments.keys().cloned().collect()
    }

    pub(crate) fn context_identities(&self) -> &[ContextIdentityObservation] {
        &self.context_identities
    }

    pub(crate) fn merged_file(&self, path: &str) -> Result<FileObservation, AdapterError> {
        let fragment_path = self.fragments.get(path).ok_or_else(|| {
            AdapterError::new(format!("missing C-family observation spool for {path:?}"))
        })?;
        let input = File::open(fragment_path).map_err(|error| {
            AdapterError::new(format!(
                "read C-family observation spool {}: {error}",
                fragment_path.display()
            ))
        })?;
        let mut merged: Option<FileObservation> = None;
        for value in serde_json::Deserializer::from_reader(BufReader::new(input))
            .into_iter::<FileObservation>()
        {
            let value = value.map_err(|error| {
                AdapterError::new(format!(
                    "decode C-family observation spool {}: {error}",
                    fragment_path.display()
                ))
            })?;
            if let Some(current) = merged.as_mut() {
                merge_file(current, value);
            } else {
                merged = Some(value);
            }
        }
        merged.ok_or_else(|| {
            AdapterError::new(format!(
                "C-family observation spool {} is empty",
                fragment_path.display()
            ))
        })
    }
}

impl Drop for StructuralObservationStore {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

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
        inventory: ScanInventory,
        workers: usize,
        shards: usize,
        merge_fan_in: usize,
    ) -> Result<StructuralObservationStore, AdapterError> {
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

        let mut store = StructuralObservationStore::create()?;
        store.append_response(self.run_structural_request(
            &repository,
            repository_root,
            inventory.owned_files,
            inventory.context_files,
            workers,
            shards,
            merge_fan_in,
        )?)?;
        Ok(store)
    }

    fn run_structural_request(
        &self,
        repository: &Path,
        repository_root: String,
        owned_files: Vec<String>,
        context_files: Vec<String>,
        workers: usize,
        shards: usize,
        merge_fan_in: usize,
    ) -> Result<StructuralResponse, AdapterError> {
        let response: StructuralResponse = self.runner.run_json(
            repository,
            &helper_arguments(),
            &helper_environment(),
            clang_protocol::PROTOCOL_VERSION,
            &StructuralRequest::new(
                repository_root,
                owned_files,
                context_files,
                workers,
                shards,
                merge_fan_in,
            ),
        )?;
        verify_helper_version(&response.helper_version)?;
        Ok(response)
    }

    #[cfg(test)]
    pub(crate) fn with_runner(runner: FrontendRunner) -> Self {
        Self { runner }
    }
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

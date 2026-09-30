use std::{
    collections::{BTreeMap, HashSet},
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{BufReader, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{
    AdapterError,
    adapters::frontend::{FrontendFrameHeader, FrontendRunner},
};

#[cfg(test)]
use super::clang_protocol::{CapabilitiesRequest, CapabilitiesResponse};
use super::{
    clang_protocol::{
        self, ContextIdentityObservation, FileObservation, StructuralMetadataFrame,
        StructuralRequest,
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
    files: BTreeMap<String, PathBuf>,
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
                        files: BTreeMap::new(),
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

    fn apply_metadata(&mut self, frame: StructuralMetadataFrame) -> Result<(), AdapterError> {
        if frame.protocol_version != clang_protocol::PROTOCOL_VERSION {
            return Err(AdapterError::new(format!(
                "C-family Clang metadata protocol mismatch: got {}, expected {}",
                frame.protocol_version,
                clang_protocol::PROTOCOL_VERSION
            )));
        }
        verify_helper_version(&frame.helper_version)?;
        let metadata = StructuralMetadata {
            protocol_version: frame.protocol_version,
            helper_version: frame.helper_version,
            clang_version: frame.clang_version,
            compilation_database: frame.compilation_database,
        };
        if self.metadata.replace(metadata).is_some() {
            return Err(AdapterError::new(
                "C-family Clang emitted duplicate structural metadata frame",
            ));
        }
        for translation_unit in frame.translation_units {
            self.translation_units
                .insert(format!("{translation_unit:?}"));
        }
        self.context_identities = frame.context_identities;
        Ok(())
    }

    fn store_file_payload(
        &mut self,
        path: &str,
        payload: &mut dyn Read,
    ) -> Result<(), AdapterError> {
        if path.is_empty() || self.files.contains_key(path) {
            return Err(AdapterError::new(format!(
                "C-family Clang emitted duplicate or empty file frame path {path:?}"
            )));
        }
        let spool = self.root.join(format!("file-{:06}.json", self.files.len()));
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&spool)
            .map_err(|error| {
                AdapterError::new(format!(
                    "create C-family observation spool {}: {error}",
                    spool.display()
                ))
            })?;
        std::io::copy(payload, &mut output).map_err(|error| {
            AdapterError::new(format!(
                "copy C-family observation frame to {}: {error}",
                spool.display()
            ))
        })?;
        self.files.insert(path.to_owned(), spool);
        Ok(())
    }

    fn finish(&self) -> Result<(), AdapterError> {
        if self.metadata.is_none() {
            return Err(AdapterError::new(
                "C-family Clang structural stream omitted metadata frame",
            ));
        }
        Ok(())
    }

    pub(crate) fn file_count(&self) -> usize {
        self.files.len()
    }

    pub(crate) fn translation_unit_count(&self) -> usize {
        self.translation_units.len()
    }

    pub(crate) fn file_paths(&self) -> Vec<String> {
        self.files.keys().cloned().collect()
    }

    pub(crate) fn context_identities(&self) -> &[ContextIdentityObservation] {
        &self.context_identities
    }

    pub(crate) fn file(&self, path: &str) -> Result<FileObservation, AdapterError> {
        let spool = self.files.get(path).ok_or_else(|| {
            AdapterError::new(format!("missing C-family observation spool for {path:?}"))
        })?;
        let input = File::open(spool).map_err(|error| {
            AdapterError::new(format!(
                "read C-family observation spool {}: {error}",
                spool.display()
            ))
        })?;
        let value: FileObservation =
            serde_json::from_reader(BufReader::new(input)).map_err(|error| {
                AdapterError::new(format!(
                    "decode C-family observation spool {}: {error}",
                    spool.display()
                ))
            })?;
        if value.path != path {
            return Err(AdapterError::new(format!(
                "C-family file frame path mismatch: header {path:?}, payload {:?}",
                value.path
            )));
        }
        Ok(value)
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
        let request = StructuralRequest::new(
            repository_root,
            inventory.owned_files,
            inventory.context_files,
            workers,
            shards,
            merge_fan_in,
        );
        self.runner.run_framed(
            &repository,
            &helper_arguments(),
            &helper_environment(),
            clang_protocol::PROTOCOL_VERSION,
            &request,
            |header: FrontendFrameHeader, payload| match header.kind.as_str() {
                "metadata" => {
                    if header.path.is_some() {
                        return Err(AdapterError::new(
                            "C-family metadata frame must not include a path",
                        ));
                    }
                    let metadata: StructuralMetadataFrame = serde_json::from_reader(payload)
                        .map_err(|error| {
                            AdapterError::new(format!(
                                "decode C-family structural metadata frame: {error}"
                            ))
                        })?;
                    store.apply_metadata(metadata)
                }
                "file" => {
                    let path = header.path.ok_or_else(|| {
                        AdapterError::new("C-family file frame is missing its path")
                    })?;
                    store.store_file_payload(&path, payload)
                }
                other => Err(AdapterError::new(format!(
                    "unsupported C-family structural frame kind {other:?}"
                ))),
            },
        )?;
        store.finish()?;
        Ok(store)
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

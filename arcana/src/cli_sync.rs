use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use arcana::lexicon::{LexiconSnapshotError, LexiconSnapshotMetadata};
use arcana::repository::RepositorySnapshot;

use crate::cli::SyncCommand;
use crate::cli_commands::CliCommandError;
use crate::cli_sync_build::{SnapshotWrite, build_snapshot, read_compatibility_warnings};
use crate::cli_sync_state::{SyncLock, replace_file};
use crate::repository_state;

pub fn run_sync(command: &SyncCommand) -> Result<String, SyncError> {
    let lexicon_root = storage_root(&command.lexicon, ".lexicon");
    repository_state::prepare(&command.state, &lexicon_root)?;
    let _lock = SyncLock::acquire(&command.state)?;
    let current = LexiconSnapshotMetadata::current(&lexicon_root)?;
    fs::create_dir_all(command.state.join("snapshots"))?;

    let output = snapshot_directory(&command.state, current.id())?;
    let previous_id = read_current(&command.state)?;
    let result = if complete_snapshot(&output, current.id()) {
        SnapshotWrite {
            mode: "existing",
            compatibility_warnings: read_compatibility_warnings(&output)?,
        }
    } else {
        if output.try_exists()? {
            fs::remove_dir_all(&output)?;
        }
        build_snapshot(
            &lexicon_root,
            &command.state,
            &output,
            previous_id.as_deref(),
            &current,
        )?
    };
    for warning in &result.compatibility_warnings {
        eprintln!("arcana sync WARNING: {warning}");
    }
    publish_current(&command.state, current.id())?;
    if command.register {
        register_consumer(&lexicon_root, &command.state)?;
    }
    Ok(format!(
        "synced Lexicon snapshot {} mode={} registered={} compatibility_warnings={}\n",
        current.id(),
        result.mode,
        command.register,
        result.compatibility_warnings.len()
    ))
}

fn complete_snapshot(output: &Path, lexicon_id: &str) -> bool {
    let source = fs::read_to_string(output.join("lexicon.snapshot"));
    source.is_ok_and(|source| source.trim() == lexicon_id)
        && RepositorySnapshot::open(output.join("repository.manifest")).is_ok()
}

fn register_consumer(lexicon_root: &Path, state: &Path) -> Result<(), SyncError> {
    let consumer_root = lexicon_root.join("consumers");
    fs::create_dir_all(&consumer_root)?;
    let command = std::env::current_exe()?;
    let lexicon = absolute(lexicon_root)?;
    let state = absolute(state)?;
    let definition = serde_json::json!({
        "version": 1,
        "command": command.to_string_lossy(),
        "args": [
            "sync",
            "--lexicon",
            lexicon.to_string_lossy(),
            "--state",
            state.to_string_lossy()
        ]
    });
    let mut bytes = serde_json::to_vec_pretty(&definition)?;
    bytes.push(b'\n');
    replace_file(&consumer_root.join("arcana.json"), &bytes)?;
    Ok(())
}

fn publish_current(state: &Path, id: &str) -> Result<(), SyncError> {
    replace_file(&state.join("CURRENT"), format!("{id}\n").as_bytes())?;
    Ok(())
}

fn read_current(state: &Path) -> Result<Option<String>, SyncError> {
    match fs::read_to_string(state.join("CURRENT")) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value.trim().to_owned())),
        Ok(_) => Err(SyncError::InvalidState(
            "Arcana CURRENT is empty".to_owned(),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn snapshot_directory(state: &Path, id: &str) -> Result<PathBuf, SyncError> {
    let digest = id
        .strip_prefix("sha256:")
        .filter(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
        .ok_or_else(|| SyncError::InvalidState(format!("invalid Lexicon snapshot ID {id}")))?;
    Ok(state.join("snapshots").join(digest))
}

pub(crate) fn storage_root(path: &Path, directory: &str) -> PathBuf {
    if path
        .file_name()
        .is_some_and(|name| name == OsStr::new(directory))
        || (path.join("CURRENT").is_file() && path.join("snapshots").is_dir())
    {
        path.to_owned()
    } else {
        path.join(directory)
    }
}

fn absolute(path: &Path) -> Result<PathBuf, SyncError> {
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

#[derive(Debug)]
pub enum SyncError {
    Io(io::Error),
    Json(serde_json::Error),
    Lexicon(LexiconSnapshotError),
    Command(CliCommandError),
    InvalidState(String),
}

impl fmt::Display for SyncError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Json(error) => error.fmt(formatter),
            Self::Lexicon(error) => error.fmt(formatter),
            Self::Command(error) => error.fmt(formatter),
            Self::InvalidState(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<io::Error> for SyncError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for SyncError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<LexiconSnapshotError> for SyncError {
    fn from(error: LexiconSnapshotError) -> Self {
        Self::Lexicon(error)
    }
}

impl From<CliCommandError> for SyncError {
    fn from(error: CliCommandError) -> Self {
        Self::Command(error)
    }
}

impl From<arcana::storage::QueryError> for SyncError {
    fn from(error: arcana::storage::QueryError) -> Self {
        Self::Command(error.into())
    }
}

impl From<arcana::repository::RepositoryCompileError> for SyncError {
    fn from(error: arcana::repository::RepositoryCompileError) -> Self {
        Self::Command(error.into())
    }
}

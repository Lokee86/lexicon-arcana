use std::fs;
use std::time::SystemTime;

use serde::Deserialize;

use super::export::operation;
use super::store::validate_storage_id;
use super::{StorageError, Store};

pub(super) struct SnapshotFile {
    pub(super) id: String,
    modified: SystemTime,
}

#[derive(Deserialize)]
struct ConsumerPin {
    snapshot_id: String,
}

pub(super) fn list_snapshots(store: &Store) -> Result<Vec<SnapshotFile>, StorageError> {
    let directory = store.root().join("snapshots");
    let entries = fs::read_dir(&directory)
        .map_err(|error| operation(format!("read Lexicon snapshots: {error}")))?;
    let mut result = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let stem = entry
            .path()
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| operation("snapshot manifest filename is not UTF-8"))?
            .to_owned();
        let id = format!("sha256:{stem}");
        validate_storage_id(&id).map_err(|_| {
            operation(format!(
                "invalid Lexicon snapshot manifest filename {:?}",
                entry.file_name()
            ))
        })?;
        result.push(SnapshotFile {
            id,
            modified: entry.metadata()?.modified()?,
        });
    }
    result.sort_by(|left, right| {
        right
            .modified
            .cmp(&left.modified)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(result)
}

pub(super) fn read_consumer_pins(store: &Store) -> Result<Vec<String>, StorageError> {
    let directory = store.root().join("consumer-state");
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(operation(format!("read Lexicon consumer state: {error}")));
        }
    };
    let mut pins = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let path = entry.path();
        let data = fs::read(&path).map_err(|error| {
            operation(format!(
                "read Lexicon consumer pin {}: {error}",
                path.display()
            ))
        })?;
        let pin: ConsumerPin = serde_json::from_slice(&data).map_err(|error| {
            operation(format!(
                "decode Lexicon consumer pin {}: {error}",
                path.display()
            ))
        })?;
        validate_storage_id(&pin.snapshot_id).map_err(|_| {
            operation(format!(
                "Lexicon consumer pin {} has invalid snapshot_id",
                path.display()
            ))
        })?;
        pins.push(pin.snapshot_id);
    }
    Ok(pins)
}

pub(super) fn list_objects(store: &Store) -> Result<Vec<String>, StorageError> {
    let root = store.root().join("objects");
    let shards = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(operation(format!("read Lexicon objects: {error}"))),
    };
    let mut result = Vec::new();
    for shard in shards {
        let shard = shard?;
        let shard_name = shard.file_name().to_string_lossy().into_owned();
        if !shard.file_type()?.is_dir() || !valid_hex_pair(&shard_name) {
            continue;
        }
        for entry in fs::read_dir(shard.path())? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let id = format!("sha256:{shard_name}{}", entry.file_name().to_string_lossy());
            if validate_storage_id(&id).is_ok() {
                result.push(id);
            }
        }
    }
    result.sort();
    Ok(result)
}

fn valid_hex_pair(value: &str) -> bool {
    value.len() == 2
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

use std::collections::BTreeMap;

use super::{LexiconSnapshotError, snapshot};

/// Lightweight metadata for one immutable Lexicon analysis state.
///
/// This verifies and indexes the content-addressed snapshot manifest without
/// reading or decoding any referenced fact objects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexiconSnapshotMetadata {
    pub(super) id: String,
    pub(super) files: BTreeMap<(String, String), String>,
    pub(super) shared_objects: BTreeMap<String, Option<String>>,
}

impl LexiconSnapshotMetadata {
    /// Reads and verifies only the manifest named by `.lexicon/CURRENT`.
    pub fn current(root: impl AsRef<std::path::Path>) -> Result<Self, LexiconSnapshotError> {
        snapshot::current_metadata(root)
    }

    /// Reads and verifies only one immutable snapshot manifest.
    pub fn load(root: impl AsRef<std::path::Path>, id: &str) -> Result<Self, LexiconSnapshotError> {
        snapshot::load_metadata(root, id)
    }

    /// Returns the SHA-256 snapshot identity, including its `sha256:` prefix.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Reports whether any language-level shared fact object changed.
    pub fn shared_objects_changed(&self, previous: &Self) -> bool {
        self.shared_objects != previous.shared_objects
    }

    /// Compares file object identities against an earlier snapshot.
    pub fn changed_paths(&self, previous: &Self) -> LexiconPathChanges {
        changed_paths(&self.files, &previous.files)
    }
}

fn changed_paths(
    current: &BTreeMap<(String, String), String>,
    previous: &BTreeMap<(String, String), String>,
) -> LexiconPathChanges {
    let mut added = Vec::new();
    let mut changed = Vec::new();
    let mut removed = Vec::new();

    for ((language, path), object_id) in current {
        match previous.get(&(language.clone(), path.clone())) {
            None => added.push(path.clone()),
            Some(previous_id) if previous_id != object_id => changed.push(path.clone()),
            Some(_) => {}
        }
    }
    for (language, path) in previous.keys() {
        if !current.contains_key(&(language.clone(), path.clone())) {
            removed.push(path.clone());
        }
    }
    added.sort_unstable();
    added.dedup();
    changed.sort_unstable();
    changed.dedup();
    removed.sort_unstable();
    removed.dedup();
    LexiconPathChanges {
        added,
        changed,
        removed,
    }
}

/// File paths whose content-addressed fact objects differ between snapshots.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LexiconPathChanges {
    pub added: Vec<String>,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
}

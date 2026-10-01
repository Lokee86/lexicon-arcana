use std::collections::{BTreeMap, BTreeSet};

use super::StorageError;
use super::dependency_index_model::FileTopology;
use super::dependency_index_partition::Partitions;

pub(super) fn update_lookup(
    index: &mut Partitions<'_, BTreeSet<String>>,
    path: &str,
    old: &BTreeSet<String>,
    new: &BTreeSet<String>,
) -> Result<(), StorageError> {
    for key in old.union(new) {
        let mut users = index.get(key)?.unwrap_or_default();
        if new.contains(key) {
            users.insert(path.to_owned());
        } else {
            users.remove(path);
        }
        index.set(key, (!users.is_empty()).then_some(users))?;
    }
    Ok(())
}

/// Recompute only changed files and unchanged sources that reference nodes
/// whose ownership might have moved. Rewire both directions consistently.
pub(super) fn refresh_links(
    files: &mut Partitions<'_, FileTopology>,
    owners: &mut Partitions<'_, String>,
    affected: &BTreeSet<String>,
    updated: &BTreeMap<String, Option<FileTopology>>,
    known: &BTreeSet<String>,
) -> Result<(), StorageError> {
    let mut previous = BTreeMap::new();
    for path in affected {
        previous.insert(path.clone(), files.get(path)?);
    }

    // Remove each affected source's old outgoing reverse links first.
    for (source, previous) in &previous {
        if let Some(previous) = previous {
            for target in &previous.forward {
                if let Some(mut data) = files.get(target)? {
                    data.reverse.remove(source);
                    files.set(target, Some(data))?;
                }
            }
        }
    }

    // Replace changed file evidence, retaining incoming references from
    // unaffected owners after their outgoing edge removals.
    for (path, next) in updated {
        if let Some(mut next) = next.clone() {
            next.reverse = files
                .get(path)?
                .map_or_else(BTreeSet::new, |data| data.reverse);
            files.set(path, Some(next))?;
        } else {
            files.set(path, None)?;
        }
    }

    let mut new_links = Vec::new();
    for path in affected {
        if !known.contains(path) {
            continue;
        }
        let Some(mut data) = files.get(path)? else {
            return Err(StorageError::Materialization(format!(
                "dependency index missing retained file {path:?}"
            )));
        };
        data.forward.clear();
        for node in &data.referenced_nodes {
            if let Some(target) = owners.get(node)?
                && target != *path
                && known.contains(&target)
            {
                data.forward.insert(target);
            }
        }
        new_links.extend(
            data.forward
                .iter()
                .map(|target| (path.clone(), target.clone())),
        );
        files.set(path, Some(data))?;
    }
    for (source, target) in new_links {
        let Some(mut data) = files.get(&target)? else {
            return Err(StorageError::Materialization(format!(
                "dependency target {target:?} is missing"
            )));
        };
        data.reverse.insert(source);
        files.set(&target, Some(data))?;
    }
    Ok(())
}

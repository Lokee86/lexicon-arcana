use std::collections::BTreeMap;
use std::path::Path;

use crate::synthetic::NodeId;

use super::{NodeFact, NodeKey, NodeKind, RepositoryPathError, normalize_repository_path};

#[path = "catalogue_file.rs"]
mod catalogue_file;

pub use catalogue_file::CatalogueError;

/// One dense graph node and its repository metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogueEntry {
    pub node_id: NodeId,
    pub fact: NodeFact,
}

/// An immutable, validated index of compiled repository node metadata.
///
/// Name and path indexes store only dense node IDs. Their strings remain owned
/// once by catalogue entries instead of being cloned into B-tree map keys.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryCatalogue {
    entries: Vec<CatalogueEntry>,
    node_ids_by_key: BTreeMap<NodeKey, NodeId>,
    node_ids_by_name: Vec<NodeId>,
    node_ids_by_path: Vec<NodeId>,
    node_ids_by_kind: BTreeMap<NodeKind, Vec<NodeId>>,
}

impl RepositoryCatalogue {
    pub fn new(mut entries: Vec<CatalogueEntry>) -> Result<Self, CatalogueError> {
        for entry in &entries {
            validate_fact(&entry.fact)?;
        }
        entries.sort_unstable_by_key(|entry| entry.node_id);
        for (index, entry) in entries.iter().enumerate() {
            let expected = u32::try_from(index).map_err(|_| CatalogueError::NodeIdOverflow)?;
            if entry.node_id.0 != expected {
                return Err(CatalogueError::NonDenseNodeId {
                    expected,
                    found: entry.node_id.0,
                });
            }
            if index > 0 && entries[index - 1].fact.key == entry.fact.key {
                return Err(CatalogueError::DuplicateNodeKey {
                    key: entry.fact.key,
                });
            }
        }

        let mut node_ids_by_key = BTreeMap::new();
        for item in &entries {
            node_ids_by_key.insert(item.fact.key, item.node_id);
        }

        let mut node_ids_by_name = dense_ids(entries.len())?;
        node_ids_by_name.sort_unstable_by(|left, right| {
            entry(&entries, *left)
                .fact
                .name
                .cmp(&entry(&entries, *right).fact.name)
                .then_with(|| left.cmp(right))
        });

        let mut node_ids_by_path = dense_ids(entries.len())?;
        node_ids_by_path.sort_unstable_by(|left, right| {
            entry(&entries, *left)
                .fact
                .path
                .cmp(&entry(&entries, *right).fact.path)
                .then_with(|| left.cmp(right))
        });

        let mut node_ids_by_kind = BTreeMap::new();
        for item in &entries {
            node_ids_by_kind
                .entry(item.fact.kind.clone())
                .or_insert_with(Vec::new)
                .push(item.node_id);
        }

        Ok(Self {
            entries,
            node_ids_by_key,
            node_ids_by_name,
            node_ids_by_path,
            node_ids_by_kind,
        })
    }

    pub fn entries(&self) -> &[CatalogueEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn lookup_by_key(&self, key: NodeKey) -> Option<&CatalogueEntry> {
        self.node_id_by_key(key)
            .and_then(|node_id| self.entry(node_id))
    }

    pub fn lookup_by_path(&self, path: &str) -> Result<Vec<&CatalogueEntry>, CatalogueError> {
        let path = normalize_repository_path(path).map_err(CatalogueError::InvalidPath)?;
        let node_ids = self.node_ids_by_path(&path)?;
        Ok(node_ids
            .iter()
            .filter_map(|&node_id| self.entry(node_id))
            .collect())
    }

    pub fn lookup_by_name(&self, name: &str) -> Vec<&CatalogueEntry> {
        self.node_ids_by_name(name)
            .iter()
            .filter_map(|&node_id| self.entry(node_id))
            .collect()
    }

    pub fn node_id_by_key(&self, key: NodeKey) -> Option<NodeId> {
        self.node_ids_by_key.get(&key).copied()
    }

    pub fn node_ids_by_name(&self, name: &str) -> &[NodeId] {
        let start = self
            .node_ids_by_name
            .partition_point(|id| self.entry_unchecked(*id).fact.name.as_str() < name);
        let end = start
            + self.node_ids_by_name[start..]
                .partition_point(|id| self.entry_unchecked(*id).fact.name.as_str() == name);
        &self.node_ids_by_name[start..end]
    }

    pub fn node_ids_by_path(&self, path: &str) -> Result<&[NodeId], CatalogueError> {
        let path = normalize_repository_path(path).map_err(CatalogueError::InvalidPath)?;
        let start = self
            .node_ids_by_path
            .partition_point(|id| self.entry_unchecked(*id).fact.path.as_str() < path.as_str());
        let end = start
            + self.node_ids_by_path[start..].partition_point(|id| {
                self.entry_unchecked(*id).fact.path.as_str() == path.as_str()
            });
        Ok(&self.node_ids_by_path[start..end])
    }

    pub fn node_ids_by_kind(&self, kind: &NodeKind) -> &[NodeId] {
        self.node_ids_by_kind.get(kind).map_or(&[], Vec::as_slice)
    }

    /// Returns IDs for a path and its descendants using the sorted path index.
    pub fn node_ids_by_path_prefix(&self, prefix: &str) -> Result<Vec<NodeId>, CatalogueError> {
        let prefix = normalize_repository_path(prefix).map_err(CatalogueError::InvalidPath)?;
        let upper_bound = format!("{prefix}0");
        let start = self
            .node_ids_by_path
            .partition_point(|id| self.entry_unchecked(*id).fact.path.as_str() < prefix.as_str());
        let end = self.node_ids_by_path.partition_point(|id| {
            self.entry_unchecked(*id).fact.path.as_str() < upper_bound.as_str()
        });
        let mut node_ids = self.node_ids_by_path[start..end]
            .iter()
            .copied()
            .filter(|id| {
                let path = self.entry_unchecked(*id).fact.path.as_str();
                path == prefix
                    || path
                        .strip_prefix(&prefix)
                        .is_some_and(|suffix| suffix.starts_with('/'))
            })
            .collect::<Vec<_>>();
        node_ids.sort_unstable();
        Ok(node_ids)
    }

    fn entry(&self, node_id: NodeId) -> Option<&CatalogueEntry> {
        self.entries
            .get(node_id.0 as usize)
            .filter(|entry| entry.node_id == node_id)
    }

    fn entry_unchecked(&self, node_id: NodeId) -> &CatalogueEntry {
        &self.entries[node_id.0 as usize]
    }

    pub fn encode(&self) -> Result<String, CatalogueError> {
        catalogue_file::encode(self)
    }

    pub fn decode(input: &str) -> Result<Self, CatalogueError> {
        catalogue_file::decode(input)
    }

    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), CatalogueError> {
        catalogue_file::write(path, self)
    }

    pub fn read(path: impl AsRef<Path>) -> Result<Self, CatalogueError> {
        catalogue_file::read(path)
    }
}

pub fn write_catalogue(
    path: impl AsRef<Path>,
    catalogue: &RepositoryCatalogue,
) -> Result<(), CatalogueError> {
    catalogue.write(path)
}

pub fn read_catalogue(path: impl AsRef<Path>) -> Result<RepositoryCatalogue, CatalogueError> {
    RepositoryCatalogue::read(path)
}

fn dense_ids(count: usize) -> Result<Vec<NodeId>, CatalogueError> {
    (0..count)
        .map(|index| {
            u32::try_from(index)
                .map(NodeId)
                .map_err(|_| CatalogueError::NodeIdOverflow)
        })
        .collect()
}

fn entry(entries: &[CatalogueEntry], id: NodeId) -> &CatalogueEntry {
    &entries[id.0 as usize]
}

fn validate_fact(fact: &NodeFact) -> Result<(), CatalogueError> {
    if let Some(identity) = &fact.external_identity {
        let Some(digest) = identity.strip_prefix("sha256:") else {
            return Err(CatalogueError::InvalidExternalIdentity);
        };
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(CatalogueError::InvalidExternalIdentity);
        }
    }
    let normalized = normalize_repository_path(&fact.path).map_err(CatalogueError::InvalidPath)?;
    if normalized != fact.path {
        return Err(CatalogueError::InvalidPath(
            RepositoryPathError::InvalidComponent,
        ));
    }
    if let Some(span) = &fact.span {
        let normalized =
            normalize_repository_path(&span.path).map_err(CatalogueError::InvalidPath)?;
        if normalized != span.path {
            return Err(CatalogueError::InvalidPath(
                RepositoryPathError::InvalidComponent,
            ));
        }
    }
    Ok(())
}

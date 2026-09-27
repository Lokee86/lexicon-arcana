use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

use super::{
    EdgeFact, NodeFact, NodeKey, NodeKind, RepositoryFacts, RepositoryPathError,
    UnresolvedReferenceFact, normalize_repository_path,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FactPartitions {
    pub shared: RepositoryFacts,
    pub files: BTreeMap<String, RepositoryFacts>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FactOwnershipError {
    InvalidPath(RepositoryPathError),
    DuplicateNodeOwner {
        key: NodeKey,
        first: String,
        second: String,
    },
}

impl fmt::Display for FactOwnershipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(error) => error.fmt(formatter),
            Self::DuplicateNodeOwner { key, first, second } => write!(
                formatter,
                "node key {key:?} is owned by both '{first}' and '{second}'"
            ),
        }
    }
}

impl std::error::Error for FactOwnershipError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPath(error) => Some(error),
            Self::DuplicateNodeOwner { .. } => None,
        }
    }
}

pub fn partition_facts(facts: &RepositoryFacts) -> Result<FactPartitions, FactOwnershipError> {
    let mut partitions = FactPartitions::default();
    let mut node_owners = BTreeMap::new();

    for node in &facts.nodes {
        let owner = node_owner(node)?;
        if let Some(path) = &owner {
            if let Some(previous) = node_owners.insert(node.key, path.clone())
                && previous != *path
            {
                return Err(FactOwnershipError::DuplicateNodeOwner {
                    key: node.key,
                    first: previous,
                    second: path.clone(),
                });
            }
            partitions
                .files
                .entry(path.clone())
                .or_default()
                .nodes
                .push(node.clone());
        } else {
            partitions.shared.nodes.push(node.clone());
        }
    }

    for edge in &facts.edges {
        if let Some(path) = edge_owner(edge, &node_owners)? {
            partitions
                .files
                .entry(path)
                .or_default()
                .edges
                .push(edge.clone());
        } else {
            partitions.shared.edges.push(edge.clone());
        }
    }

    for reference in &facts.unresolved {
        if let Some(path) = unresolved_owner(reference, &node_owners)? {
            partitions
                .files
                .entry(path)
                .or_default()
                .unresolved
                .push(reference.clone());
        } else {
            partitions.shared.unresolved.push(reference.clone());
        }
    }

    canonicalize(&mut partitions.shared);
    for partition in partitions.files.values_mut() {
        canonicalize(partition);
    }
    Ok(partitions)
}

pub fn replace_changed_files(
    base: &RepositoryFacts,
    replacement: &RepositoryFacts,
    changed_paths: &[String],
) -> Result<RepositoryFacts, FactOwnershipError> {
    let base = partition_facts(base)?;
    let replacement = partition_facts(replacement)?;
    let changed = changed_paths
        .iter()
        .map(|path| normalize_repository_path(path).map_err(FactOwnershipError::InvalidPath))
        .collect::<Result<BTreeSet<_>, _>>()?;

    let mut merged = replacement.shared;
    for (path, facts) in base.files {
        if !changed.contains(&path) {
            append(&mut merged, facts);
        }
    }
    for path in changed {
        if let Some(facts) = replacement.files.get(&path) {
            append(&mut merged, facts.clone());
        }
    }
    canonicalize(&mut merged);
    Ok(merged)
}

/// Replaces changed-file facts while moving every unchanged base record into
/// the result. Only shared and changed-file records selected from the
/// replacement set are cloned.
pub fn replace_changed_files_owned_base(
    mut base: RepositoryFacts,
    replacement: &RepositoryFacts,
    changed_paths: &[String],
) -> Result<RepositoryFacts, FactOwnershipError> {
    let changed = changed_paths
        .iter()
        .map(|path| normalize_repository_path(path).map_err(FactOwnershipError::InvalidPath))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let base_owners = collect_node_owners(&base.nodes)?;
    let replacement_owners = collect_node_owners(&replacement.nodes)?;
    let mut merged = RepositoryFacts::default();

    for node in base.nodes.drain(..) {
        if node_owner(&node)?.is_some_and(|path| !changed.contains(&path)) {
            merged.nodes.push(node);
        }
    }
    for edge in base.edges.drain(..) {
        if edge_owner(&edge, &base_owners)?.is_some_and(|path| !changed.contains(&path)) {
            merged.edges.push(edge);
        }
    }
    for reference in base.unresolved.drain(..) {
        if unresolved_owner(&reference, &base_owners)?.is_some_and(|path| !changed.contains(&path))
        {
            merged.unresolved.push(reference);
        }
    }

    for node in &replacement.nodes {
        let owner = node_owner(node)?;
        if owner.as_ref().is_none_or(|path| changed.contains(path)) {
            merged.nodes.push(node.clone());
        }
    }
    for edge in &replacement.edges {
        let owner = edge_owner(edge, &replacement_owners)?;
        if owner.as_ref().is_none_or(|path| changed.contains(path)) {
            merged.edges.push(edge.clone());
        }
    }
    for reference in &replacement.unresolved {
        let owner = unresolved_owner(reference, &replacement_owners)?;
        if owner.as_ref().is_none_or(|path| changed.contains(path)) {
            merged.unresolved.push(reference.clone());
        }
    }

    canonicalize(&mut merged);
    Ok(merged)
}

pub(crate) fn collect_node_owners(
    nodes: &[NodeFact],
) -> Result<BTreeMap<NodeKey, String>, FactOwnershipError> {
    let mut owners = BTreeMap::new();
    for node in nodes {
        if let Some(path) = node_owner(node)?
            && let Some(previous) = owners.insert(node.key, path.clone())
            && previous != path
        {
            return Err(FactOwnershipError::DuplicateNodeOwner {
                key: node.key,
                first: previous,
                second: path,
            });
        }
    }
    Ok(owners)
}

pub(crate) fn node_owner(node: &NodeFact) -> Result<Option<String>, FactOwnershipError> {
    if let Some(span) = &node.span {
        return normalize_repository_path(&span.path)
            .map(Some)
            .map_err(FactOwnershipError::InvalidPath);
    }
    if matches!(
        node.kind,
        NodeKind::Repository | NodeKind::Directory | NodeKind::Module | NodeKind::Namespace
    ) || Path::new(&node.path).extension().is_none()
    {
        return Ok(None);
    }
    normalize_repository_path(&node.path)
        .map(Some)
        .map_err(FactOwnershipError::InvalidPath)
}

pub(crate) fn edge_owner(
    edge: &EdgeFact,
    node_owners: &BTreeMap<NodeKey, String>,
) -> Result<Option<String>, FactOwnershipError> {
    if let Some(span) = &edge.span {
        return normalize_repository_path(&span.path)
            .map(Some)
            .map_err(FactOwnershipError::InvalidPath);
    }
    Ok(node_owners
        .get(&edge.source)
        .or_else(|| node_owners.get(&edge.target))
        .cloned())
}

pub(crate) fn unresolved_owner(
    reference: &UnresolvedReferenceFact,
    node_owners: &BTreeMap<NodeKey, String>,
) -> Result<Option<String>, FactOwnershipError> {
    if let Some(span) = &reference.span {
        return normalize_repository_path(&span.path)
            .map(Some)
            .map_err(FactOwnershipError::InvalidPath);
    }
    Ok(node_owners.get(&reference.source).cloned())
}

fn append(target: &mut RepositoryFacts, source: RepositoryFacts) {
    target.nodes.extend(source.nodes);
    target.edges.extend(source.edges);
    target.unresolved.extend(source.unresolved);
}

fn canonicalize(facts: &mut RepositoryFacts) {
    facts.nodes.sort_unstable();
    facts.nodes.dedup();
    facts.edges.sort_unstable();
    facts.edges.dedup();
    facts.unresolved.sort_unstable();
    facts.unresolved.dedup();
}

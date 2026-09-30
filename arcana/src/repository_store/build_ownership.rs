use std::collections::BTreeMap;
use std::path::Path;

use crate::repository::{NodeKey, NodeKind};

use super::build::{CompactOwnershipRecord, CompactRepositoryBuild};
use super::canonical::{Contribution, ContributionKind};
use super::format::node_kind_from_code;
use super::{RepositoryStoreWriteError, StringId};

pub(super) fn build_ownership(
    build: &CompactRepositoryBuild,
) -> Result<(Vec<CompactOwnershipRecord>, Vec<Contribution>), RepositoryStoreWriteError> {
    let mut node_owners = BTreeMap::<NodeKey, StringId>::new();
    let mut ownership = BTreeMap::<StringId, Vec<Contribution>>::new();

    for (index, node) in build.nodes.iter().enumerate() {
        if let Some(path) = compact_node_owner(&build.strings, node)? {
            if let Some(previous) = node_owners.insert(node.key, path)
                && previous != path
            {
                return Err(RepositoryStoreWriteError::DuplicateCompactNodeOwner { key: node.key });
            }
            add(&mut ownership, path, ContributionKind::Node, index)?;
        }
    }
    for (index, edge) in build.edges.iter().enumerate() {
        if let Some(path) = edge
            .span
            .map(|span| span.path)
            .or_else(|| node_owners.get(&edge.source).copied())
            .or_else(|| node_owners.get(&edge.target).copied())
        {
            add(&mut ownership, path, ContributionKind::Edge, index)?;
        }
    }
    for (index, reference) in build.unresolved.iter().enumerate() {
        if let Some(path) = reference
            .span
            .map(|span| span.path)
            .or_else(|| node_owners.get(&reference.source).copied())
        {
            add(&mut ownership, path, ContributionKind::Unresolved, index)?;
        }
    }

    flatten(ownership)
}

pub(super) fn compact_node_owner(
    strings: &super::CompactStringTable,
    node: &super::CompactNodeRecord,
) -> Result<Option<StringId>, RepositoryStoreWriteError> {
    if let Some(span) = node.span {
        return Ok(Some(span.path));
    }
    let kind = node_kind_from_code(node.kind_code)
        .ok_or(super::StoreFormatError::InvalidNodeKind(node.kind_code))?;
    let path = strings.get(node.path)?;
    if matches!(
        kind,
        NodeKind::Repository | NodeKind::Directory | NodeKind::Module | NodeKind::Namespace
    ) || Path::new(path).extension().is_none()
    {
        return Ok(None);
    }
    Ok(Some(node.path))
}

fn add(
    ownership: &mut BTreeMap<StringId, Vec<Contribution>>,
    path: StringId,
    kind: ContributionKind,
    index: usize,
) -> Result<(), RepositoryStoreWriteError> {
    let record_index =
        u64::try_from(index).map_err(|_| RepositoryStoreWriteError::TooManyContributions)?;
    ownership
        .entry(path)
        .or_default()
        .push(Contribution { kind, record_index });
    Ok(())
}

fn flatten(
    ownership: BTreeMap<StringId, Vec<Contribution>>,
) -> Result<(Vec<CompactOwnershipRecord>, Vec<Contribution>), RepositoryStoreWriteError> {
    let total = ownership
        .values()
        .try_fold(0_usize, |total, values| total.checked_add(values.len()))
        .ok_or(RepositoryStoreWriteError::TooManyContributions)?;
    let mut records = Vec::with_capacity(ownership.len());
    let mut contributions = Vec::with_capacity(total);
    for (path, values) in ownership {
        records.push(CompactOwnershipRecord {
            path,
            contribution_start: u64::try_from(contributions.len())
                .map_err(|_| RepositoryStoreWriteError::TooManyContributions)?,
            contribution_count: u64::try_from(values.len())
                .map_err(|_| RepositoryStoreWriteError::TooManyContributions)?,
        });
        contributions.extend(values);
    }
    Ok((records, contributions))
}

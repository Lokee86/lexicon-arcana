use std::path::Path;

use crate::repository::{NodeKey, NodeKind};

use super::build::{CompactOwnershipRecord, CompactRepositoryBuild};
use super::canonical::{Contribution, ContributionKind};
use super::format::node_kind_from_code;
use super::{RepositoryStoreWriteError, StringId};

pub(super) fn build_ownership(
    build: &CompactRepositoryBuild,
) -> Result<(Vec<CompactOwnershipRecord>, Vec<Contribution>), RepositoryStoreWriteError> {
    let owner_by_node = build_owner_by_node(build)?;
    let mut slots = vec![0_u64; build.strings.len()];

    count_contributions(build, &owner_by_node, &mut slots)?;
    let (records, total) = prefix_ownership(&mut slots)?;
    let mut contributions = vec![
        Contribution {
            kind: ContributionKind::Node,
            record_index: 0,
        };
        total
    ];
    fill_contributions(build, &owner_by_node, &mut slots, &mut contributions)?;

    Ok((records, contributions))
}

fn count_contributions(
    build: &CompactRepositoryBuild,
    owner_by_node: &[StringId],
    counts: &mut [u64],
) -> Result<(), RepositoryStoreWriteError> {
    for owner in owner_by_node.iter().copied().filter_map(StringId::present) {
        increment(counts, owner)?;
    }
    for edge in &build.edges {
        if let Some(owner) = edge_owner(build, owner_by_node, edge) {
            increment(counts, owner)?;
        }
    }
    for reference in &build.unresolved {
        if let Some(owner) = unresolved_owner(build, owner_by_node, reference) {
            increment(counts, owner)?;
        }
    }
    Ok(())
}

fn prefix_ownership(
    slots: &mut [u64],
) -> Result<(Vec<CompactOwnershipRecord>, usize), RepositoryStoreWriteError> {
    let owner_count = slots.iter().filter(|count| **count != 0).count();
    let mut records = Vec::with_capacity(owner_count);
    let mut total = 0_u64;

    for (path, count) in slots.iter_mut().enumerate() {
        let contribution_count = *count;
        if contribution_count == 0 {
            continue;
        }
        records.push(CompactOwnershipRecord {
            path: StringId(path as u32),
            contribution_start: total,
            contribution_count,
        });
        *count = total;
        total = total
            .checked_add(contribution_count)
            .ok_or(RepositoryStoreWriteError::TooManyContributions)?;
    }

    let total =
        usize::try_from(total).map_err(|_| RepositoryStoreWriteError::TooManyContributions)?;
    Ok((records, total))
}

fn fill_contributions(
    build: &CompactRepositoryBuild,
    owner_by_node: &[StringId],
    cursors: &mut [u64],
    output: &mut [Contribution],
) -> Result<(), RepositoryStoreWriteError> {
    for (index, owner) in owner_by_node.iter().copied().enumerate() {
        if let Some(owner) = owner.present() {
            write(output, cursors, owner, ContributionKind::Node, index)?;
        }
    }
    for (index, edge) in build.edges.iter().enumerate() {
        if let Some(owner) = edge_owner(build, owner_by_node, edge) {
            write(output, cursors, owner, ContributionKind::Edge, index)?;
        }
    }
    for (index, reference) in build.unresolved.iter().enumerate() {
        if let Some(owner) = unresolved_owner(build, owner_by_node, reference) {
            write(output, cursors, owner, ContributionKind::Unresolved, index)?;
        }
    }
    Ok(())
}

fn build_owner_by_node(
    build: &CompactRepositoryBuild,
) -> Result<Vec<StringId>, RepositoryStoreWriteError> {
    build
        .nodes
        .iter()
        .map(|node| compact_node_owner(&build.strings, node).map(StringId::optional))
        .collect()
}

fn edge_owner(
    build: &CompactRepositoryBuild,
    owner_by_node: &[StringId],
    edge: &super::CompactEdgeRecord,
) -> Option<StringId> {
    edge.span
        .map(|span| span.path)
        .or_else(|| node_owner_by_key(build, owner_by_node, edge.source))
        .or_else(|| node_owner_by_key(build, owner_by_node, edge.target))
}

fn unresolved_owner(
    build: &CompactRepositoryBuild,
    owner_by_node: &[StringId],
    reference: &super::CompactUnresolvedRecord,
) -> Option<StringId> {
    reference
        .span
        .map(|span| span.path)
        .or_else(|| node_owner_by_key(build, owner_by_node, reference.source))
}

fn node_owner_by_key(
    build: &CompactRepositoryBuild,
    owner_by_node: &[StringId],
    key: NodeKey,
) -> Option<StringId> {
    build
        .nodes
        .binary_search_by_key(&key, |node| node.key)
        .ok()
        .and_then(|index| owner_by_node[index].present())
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

fn increment(counts: &mut [u64], path: StringId) -> Result<(), RepositoryStoreWriteError> {
    let count = &mut counts[path.0 as usize];
    *count = count
        .checked_add(1)
        .ok_or(RepositoryStoreWriteError::TooManyContributions)?;
    Ok(())
}

fn write(
    output: &mut [Contribution],
    cursors: &mut [u64],
    path: StringId,
    kind: ContributionKind,
    index: usize,
) -> Result<(), RepositoryStoreWriteError> {
    let cursor = &mut cursors[path.0 as usize];
    let position =
        usize::try_from(*cursor).map_err(|_| RepositoryStoreWriteError::TooManyContributions)?;
    let record_index =
        u64::try_from(index).map_err(|_| RepositoryStoreWriteError::TooManyContributions)?;
    output[position] = Contribution { kind, record_index };
    *cursor = cursor
        .checked_add(1)
        .ok_or(RepositoryStoreWriteError::TooManyContributions)?;
    Ok(())
}

#[cfg(test)]
#[path = "build_ownership_tests.rs"]
mod tests;

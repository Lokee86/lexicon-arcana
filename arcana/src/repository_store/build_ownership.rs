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
    let owner_by_node = build_node_owners(build)?;
    let mut ownership = BTreeMap::<StringId, Vec<Contribution>>::new();

    for (index, owner) in owner_by_node.iter().copied().enumerate() {
        if let Some(path) = owner.present() {
            add(&mut ownership, path, ContributionKind::Node, index)?;
        }
    }
    for (index, edge) in build.edges.iter().enumerate() {
        let path = edge
            .span
            .map(|span| span.path)
            .or_else(|| node_owner_by_key(build, &owner_by_node, edge.source))
            .or_else(|| node_owner_by_key(build, &owner_by_node, edge.target));
        if let Some(path) = path {
            add(&mut ownership, path, ContributionKind::Edge, index)?;
        }
    }
    for (index, reference) in build.unresolved.iter().enumerate() {
        let path = reference
            .span
            .map(|span| span.path)
            .or_else(|| node_owner_by_key(build, &owner_by_node, reference.source));
        if let Some(path) = path {
            add(&mut ownership, path, ContributionKind::Unresolved, index)?;
        }
    }

    flatten(ownership)
}

fn build_node_owners(
    build: &CompactRepositoryBuild,
) -> Result<Vec<StringId>, RepositoryStoreWriteError> {
    build
        .nodes
        .iter()
        .map(|node| node_owner(build, node).map(StringId::optional))
        .collect()
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

fn node_owner(
    build: &CompactRepositoryBuild,
    node: &super::CompactNodeRecord,
) -> Result<Option<StringId>, RepositoryStoreWriteError> {
    if let Some(span) = node.span {
        return Ok(Some(span.path));
    }
    let kind = node_kind_from_code(node.kind_code)
        .ok_or(super::StoreFormatError::InvalidNodeKind(node.kind_code))?;
    let path = build.strings.get(node.path)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository_store::writer_test_support::sample_facts;

    #[test]
    fn node_owners_are_dense_and_aligned_to_canonical_nodes() {
        let build = CompactRepositoryBuild::from_facts(&sample_facts()).unwrap();
        let owner_by_node = build_node_owners(&build).unwrap();

        assert_eq!(owner_by_node.len(), build.nodes.len());
        for (index, node) in build.nodes.iter().enumerate() {
            assert_eq!(
                node_owner_by_key(&build, &owner_by_node, node.key),
                owner_by_node[index].present()
            );
        }
    }
}

use super::*;
use crate::repository_store::canonical::CanonicalFacts;
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

#[test]
fn flat_contributions_match_canonical_order_and_exact_capacity() {
    let facts = sample_facts();
    let canonical = CanonicalFacts::prepare(&facts).unwrap();
    let build = CompactRepositoryBuild::from_facts(&facts).unwrap();

    assert_eq!(build.ownership.capacity(), build.ownership.len());
    assert_eq!(build.contributions.capacity(), build.contributions.len());
    assert_eq!(build.ownership.len(), canonical.ownership.len());

    for (record, (path, expected)) in build.ownership.iter().zip(&canonical.ownership) {
        assert_eq!(build.strings.get(record.path).unwrap(), path);
        let start = record.contribution_start as usize;
        let end = start + record.contribution_count as usize;
        assert_eq!(&build.contributions[start..end], expected.as_slice());
    }
}

use super::*;
use crate::repository_store::canonical::CanonicalFacts;
use crate::repository_store::writer_test_support::sample_facts;

#[test]
fn owner_by_node_is_dense_and_aligned_to_canonical_nodes() {
    let build = CompactRepositoryBuild::from_facts(&sample_facts()).unwrap();
    let owner_by_node = build_owner_by_node(&build).unwrap();

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

#[test]
fn final_ownership_interface_is_sorted_contiguous_and_index_valid() {
    let build = CompactRepositoryBuild::from_facts(&sample_facts()).unwrap();

    assert!(
        build
            .ownership
            .windows(2)
            .all(|pair| pair[0].path < pair[1].path)
    );

    let mut expected_start = 0_u64;
    for owner in &build.ownership {
        assert_eq!(owner.contribution_start, expected_start);
        assert!(owner.contribution_count > 0);

        let start = owner.contribution_start as usize;
        let end = start + owner.contribution_count as usize;
        let values = &build.contributions[start..end];

        assert!(values.windows(2).all(|pair| {
            pair[0].kind < pair[1].kind
                || (pair[0].kind == pair[1].kind && pair[0].record_index < pair[1].record_index)
        }));

        for contribution in values {
            let upper_bound = match contribution.kind {
                ContributionKind::Node => build.nodes.len(),
                ContributionKind::Edge => build.edges.len(),
                ContributionKind::Unresolved => build.unresolved.len(),
            };
            assert!((contribution.record_index as usize) < upper_bound);
        }

        expected_start += owner.contribution_count;
    }

    assert_eq!(expected_start as usize, build.contributions.len());
}

use std::fs;

use crate::repository::RepositoryFacts;
use crate::synthetic::NodeId;

use super::super::canonical::CanonicalFacts;
use super::super::write_repository_store;
use super::super::writer_test_support::{cleanup, sample_facts, temp_path};
use super::CompactRepositoryBuild;

#[test]
fn compact_build_preserves_canonical_records_and_occurrences() {
    let facts = sample_facts();
    let build = CompactRepositoryBuild::from_facts(&facts).unwrap();
    let materialized = materialize(&build);

    let mut expected_nodes = facts.nodes.clone();
    expected_nodes.sort_unstable();
    let mut expected_edges = facts.edges.clone();
    expected_edges.sort_unstable();
    let mut expected_unresolved = facts.unresolved.clone();
    expected_unresolved.sort_unstable();

    assert_eq!(materialized.nodes, expected_nodes);
    assert_eq!(materialized.edges, expected_edges);
    assert_eq!(materialized.unresolved, expected_unresolved);
}

#[test]
fn compact_build_preserves_dense_lookup_indexes_and_ownership() {
    let facts = sample_facts();
    let canonical = CanonicalFacts::prepare(&facts).unwrap();
    let build = CompactRepositoryBuild::from_facts(&facts).unwrap();

    for (index, node) in build.nodes.iter().enumerate() {
        assert_eq!(build.node_id(node.key), Some(NodeId(index as u32)));
    }

    assert_sorted_by(&build.name_index, |id| build.nodes[id.0 as usize].name);
    assert_sorted_by(&build.path_index, |id| build.nodes[id.0 as usize].path);
    assert!(build.kind_index.windows(2).all(|pair| {
        (pair[0].kind_code, pair[0].node_id) <= (pair[1].kind_code, pair[1].node_id)
    }));

    assert_eq!(build.ownership.len(), canonical.ownership.len());
    for (record, (path, expected)) in build.ownership.iter().zip(&canonical.ownership) {
        assert_eq!(build.strings.get(record.path).unwrap(), path);
        let start = record.contribution_start as usize;
        let end = start + record.contribution_count as usize;
        assert_eq!(&build.contributions[start..end], expected.as_slice());
    }
}

#[test]
fn compact_build_round_trips_to_identical_repository_store_bytes() {
    let facts = sample_facts();
    let build = CompactRepositoryBuild::from_facts(&facts).unwrap();
    let materialized = materialize(&build);
    let original_path = temp_path("compact-build-original");
    let compact_path = temp_path("compact-build-round-trip");

    write_repository_store(&original_path, &facts).unwrap();
    write_repository_store(&compact_path, &materialized).unwrap();

    assert_eq!(
        fs::read(&original_path).unwrap(),
        fs::read(&compact_path).unwrap()
    );
    cleanup(&[&original_path, &compact_path]);
}

#[test]
fn compact_build_interns_shared_text_once() {
    let facts = sample_facts();
    let build = CompactRepositoryBuild::from_facts(&facts).unwrap();

    let path_id = build.nodes[1].path;
    assert_eq!(build.strings.get(path_id).unwrap(), "src/a.rs");
    assert!(
        build
            .nodes
            .iter()
            .filter(|node| node.path == path_id)
            .count()
            >= 2
    );
    assert!(
        build
            .edges
            .iter()
            .filter_map(|edge| edge.span)
            .any(|span| span.path == path_id)
    );
    assert!(
        build
            .unresolved
            .iter()
            .filter_map(|reference| reference.span)
            .any(|span| span.path == path_id)
    );
}

fn materialize(build: &CompactRepositoryBuild) -> RepositoryFacts {
    let mut facts = RepositoryFacts::default();
    for record in &build.nodes {
        let fact = record.to_fact(&build.strings).unwrap();
        for _ in 0..record.occurrence_count {
            facts.nodes.push(fact.clone());
        }
    }
    facts.edges.extend(
        build
            .edges
            .iter()
            .map(|record| record.to_fact(&build.strings).unwrap()),
    );
    facts.unresolved.extend(
        build
            .unresolved
            .iter()
            .map(|record| record.to_fact(&build.strings).unwrap()),
    );
    facts
}

fn assert_sorted_by(ids: &[NodeId], value: impl Fn(NodeId) -> super::super::StringId) {
    assert!(
        ids.windows(2)
            .all(|pair| { (value(pair[0]), pair[0]) <= (value(pair[1]), pair[1]) })
    );
}

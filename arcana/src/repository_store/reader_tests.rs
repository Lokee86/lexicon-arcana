use std::fs;

use crate::repository::{NodeKind, RepositoryFacts};

use super::format::{RepositoryHeader, SectionKind};
use super::writer_test_support::{cleanup, sample_facts, temp_path};
use super::{
    ContributionKindView, RepositoryStore, RepositoryStoreReadError, write_repository_store,
};

#[test]
fn reader_opens_writer_output_without_materializing_repository_facts() {
    let facts = sample_facts();
    let path = temp_path("reader-open");
    write_repository_store(&path, &facts).unwrap();

    let store = RepositoryStore::open(&path).unwrap();
    assert_eq!(store.node_count(), 4);
    assert_eq!(store.edge_count(), 3);
    assert_eq!(store.unresolved_count(), 2);

    let function = store.node(2).unwrap();
    assert_eq!(function.path().unwrap(), "src/a.rs");
    assert_eq!(function.name().unwrap(), "odd\tname\n☃");
    assert_eq!(function.occurrence_count(), 2);
    assert_eq!(function.external_identity_digest(), Some([0xab; 32]));
    assert_eq!(function.content_id().unwrap().as_u64(), 0x44);

    cleanup(&[&path]);
}

#[test]
fn reader_queries_binary_indexes_with_borrowed_strings() {
    let path = temp_path("reader-indexes");
    write_repository_store(&path, &sample_facts()).unwrap();
    let store = RepositoryStore::open(&path).unwrap();

    let by_name = store.lookup_by_name("odd\tname\n☃").unwrap();
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].node_id, 2);

    let by_path = store.lookup_by_path("src\\a.rs").unwrap();
    assert_eq!(
        by_path.iter().map(|node| node.node_id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(store.node_ids_by_kind(&NodeKind::File).unwrap(), vec![1, 3]);
    assert_eq!(store.node_ids_by_path_prefix("src").unwrap(), vec![1, 2, 3]);
    assert_eq!(
        store
            .lookup_by_key(crate::repository::NodeKey::from_u64(3))
            .unwrap()
            .unwrap()
            .node_id,
        2
    );

    let direct = store.node(2).unwrap().name().unwrap();
    let indexed = by_name[0].name().unwrap();
    assert_eq!(direct.as_ptr(), indexed.as_ptr());
    assert_eq!(direct.len(), indexed.len());

    cleanup(&[&path]);
}

#[test]
fn reader_views_round_trip_occurrence_facts_only_on_demand() {
    let facts = sample_facts();
    let path = temp_path("reader-records");
    write_repository_store(&path, &facts).unwrap();
    let store = RepositoryStore::open(&path).unwrap();

    let mut decoded = RepositoryFacts::default();
    for node_id in 0..store.node_count() {
        let node = store.node(node_id).unwrap();
        let fact = node.materialize().unwrap();
        for _ in 0..node.occurrence_count() {
            decoded.nodes.push(fact.clone());
        }
    }
    for index in 0..store.edge_count() {
        decoded
            .edges
            .push(store.edge(index).unwrap().unwrap().materialize().unwrap());
    }
    for index in 0..store.unresolved_count() {
        decoded.unresolved.push(
            store
                .unresolved(index)
                .unwrap()
                .unwrap()
                .materialize()
                .unwrap(),
        );
    }

    assert_eq!(decoded.canonicalized(), facts.canonicalized());
    cleanup(&[&path]);
}

#[test]
fn reader_ownership_locates_changed_file_records_without_fact_scan() {
    let path = temp_path("reader-ownership");
    write_repository_store(&path, &sample_facts()).unwrap();
    let store = RepositoryStore::open(&path).unwrap();

    let contributions = store
        .ownership()
        .unwrap()
        .contributions("src/a.rs")
        .unwrap();
    assert_eq!(contributions.len(), 7);
    assert_eq!(
        contributions
            .iter()
            .filter(|item| item.kind == ContributionKindView::Node)
            .count(),
        2
    );
    assert_eq!(
        contributions
            .iter()
            .filter(|item| item.kind == ContributionKindView::Edge)
            .count(),
        3
    );
    assert_eq!(
        contributions
            .iter()
            .filter(|item| item.kind == ContributionKindView::Unresolved)
            .count(),
        2
    );
    assert!(
        store
            .ownership()
            .unwrap()
            .contributions("missing.rs")
            .unwrap()
            .is_empty()
    );

    cleanup(&[&path]);
}

#[test]
fn incremental_ownership_materializes_only_requested_file() {
    let path = temp_path("reader-incremental-owned");
    write_repository_store(&path, &sample_facts()).unwrap();
    let store = RepositoryStore::open(&path).unwrap();

    assert_eq!(
        store.owned_node_keys(&["src\\a.rs".to_owned()]).unwrap(),
        vec![
            crate::repository::NodeKey::from_u64(2),
            crate::repository::NodeKey::from_u64(3)
        ]
    );
    let owned = store.owned_facts(&["src/a.rs".to_owned()]).unwrap();
    assert_eq!(owned.nodes.len(), 3);
    assert_eq!(owned.edges.len(), 3);
    assert_eq!(owned.unresolved.len(), 2);
    assert!(owned.nodes.iter().all(|node| node.path == "src/a.rs"));

    cleanup(&[&path]);
}

#[test]
fn reader_rejects_payload_corruption_before_exposing_views() {
    let path = temp_path("reader-corrupt");
    write_repository_store(&path, &sample_facts()).unwrap();
    let mut bytes = fs::read(&path).unwrap();
    let header = RepositoryHeader::decode(&bytes).unwrap();
    bytes[header.section(SectionKind::Nodes).offset as usize] ^= 0x80;

    assert!(matches!(
        RepositoryStore::from_bytes(bytes.into_boxed_slice()),
        Err(RepositoryStoreReadError::PayloadChecksum)
    ));

    cleanup(&[&path]);
}

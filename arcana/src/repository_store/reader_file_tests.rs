use std::fs;

use crate::repository::{NodeKey, repository_artifact_checksum};

use super::format::{RepositoryHeader, SectionKind};
use super::writer_test_support::{cleanup, sample_facts, temp_path};
use super::{
    RepositoryStore, RepositoryStoreFile, RepositoryStoreReadError, write_repository_store,
};

#[test]
fn file_reader_matches_incremental_owned_node_lookup_without_retaining_store_bytes() {
    let path = temp_path("reader-file-incremental");
    write_repository_store(&path, &sample_facts()).unwrap();

    let memory = RepositoryStore::open(&path).unwrap();
    let expected = memory
        .owned_node_keys(&[r"src\a.rs".to_owned(), "missing.rs".to_owned()])
        .unwrap();

    let bytes = fs::read(&path).unwrap();
    let expected_checksum = repository_artifact_checksum(&bytes);
    drop(bytes);

    let mut file = RepositoryStoreFile::open(&path).unwrap();
    assert_eq!(file.artifact_checksum(), expected_checksum);
    assert_eq!(
        file.owned_node_keys(&[r"src\a.rs".to_owned(), "missing.rs".to_owned()])
            .unwrap(),
        expected
    );
    assert_eq!(expected, vec![NodeKey::from_u64(2), NodeKey::from_u64(3)]);

    cleanup(&[&path]);
}

#[test]
fn file_reader_rejects_section_corruption_during_streaming_validation() {
    let path = temp_path("reader-file-corrupt");
    write_repository_store(&path, &sample_facts()).unwrap();
    let mut bytes = fs::read(&path).unwrap();
    let header = RepositoryHeader::decode(&bytes).unwrap();
    bytes[header.section(SectionKind::Nodes).offset as usize] ^= 0x80;
    fs::write(&path, bytes).unwrap();

    assert!(matches!(
        RepositoryStoreFile::open(&path),
        Err(RepositoryStoreReadError::SectionChecksum(
            SectionKind::Nodes
        ))
    ));

    cleanup(&[&path]);
}

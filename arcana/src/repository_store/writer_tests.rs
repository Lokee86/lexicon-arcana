use std::fs;

use crate::repository::{EdgeFact, NodeKey, RelationKind};

use super::format::{
    CONTRIBUTION_KIND_OFFSET, CONTRIBUTION_RECORD_INDEX_OFFSET, FILE_OWNERSHIP_RECORD_LEN,
    HEADER_LEN, OWNERSHIP_CONTRIBUTION_COUNT_OFFSET, OWNERSHIP_CONTRIBUTION_START_OFFSET,
    OWNERSHIP_PATH_ID_OFFSET, RepositoryHeader, SectionKind,
};
use super::record_io::{get_u32, get_u64};
use super::writer_test_support::{
    cleanup, decode_facts, sample_facts, section, temp_path, verify_checksums,
};
use super::{CompactStringTable, StringId, write_repository_store};

#[test]
fn writer_round_trips_canonical_facts_and_checksums() {
    let facts = sample_facts();
    let path = temp_path("roundtrip");
    let result = write_repository_store(&path, &facts).unwrap();
    let bytes = fs::read(&path).unwrap();

    assert_eq!(result.header.file_len, bytes.len() as u64);
    assert_eq!(decode_facts(&bytes).canonicalized(), facts.canonicalized());
    assert_eq!(
        result.header.section(SectionKind::Edges).record_count,
        facts.edges.len() as u64
    );
    assert_eq!(
        result.header.section(SectionKind::Unresolved).record_count,
        facts.unresolved.len() as u64
    );
    verify_checksums(&bytes);
    cleanup(&[&path]);
}

#[test]
fn writer_is_byte_deterministic_for_reordered_equivalent_input() {
    let facts = sample_facts();
    let mut reordered = facts.clone();
    reordered.nodes.reverse();
    reordered.edges.rotate_left(1);
    reordered.unresolved.reverse();

    let first = temp_path("deterministic-a");
    let second = temp_path("deterministic-b");
    write_repository_store(&first, &facts).unwrap();
    write_repository_store(&second, &reordered).unwrap();

    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    cleanup(&[&first, &second]);
}

#[test]
fn ownership_section_preserves_file_contribution_record_ids() {
    let facts = sample_facts();
    let path = temp_path("ownership");
    write_repository_store(&path, &facts).unwrap();
    let bytes = fs::read(&path).unwrap();
    let header = RepositoryHeader::decode(&bytes[..usize::from(HEADER_LEN)]).unwrap();

    let strings_bytes = section(&bytes, &header, SectionKind::Strings);
    let strings = CompactStringTable::decode(
        strings_bytes,
        header.section(SectionKind::Strings).record_count,
    )
    .unwrap();
    let ownership = section(&bytes, &header, SectionKind::Ownership);
    let file_count = header.section(SectionKind::Ownership).record_count as usize;
    let contribution_base = file_count * FILE_OWNERSHIP_RECORD_LEN as usize;

    let a = file_record(ownership, 0);
    assert_eq!(strings.get(StringId(a.path_id)).unwrap(), "src/a.rs");
    let contributions = &ownership[contribution_base + a.start as usize * 16
        ..contribution_base + (a.start + a.count) as usize * 16];
    let kinds = contributions
        .chunks_exact(16)
        .map(|record| {
            (
                record[CONTRIBUTION_KIND_OFFSET],
                get_u64(record, CONTRIBUTION_RECORD_INDEX_OFFSET),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![(1, 1), (1, 2), (2, 0), (2, 1), (2, 2), (3, 0), (3, 1)]
    );

    let b = file_record(ownership, 1);
    assert_eq!(strings.get(StringId(b.path_id)).unwrap(), "src/b.rs");
    assert_eq!(b.count, 1);
    cleanup(&[&path]);
}

#[test]
fn writer_preserves_exact_duplicate_edges() {
    let mut facts = sample_facts();
    facts.edges.push(EdgeFact {
        source: NodeKey::from_u64(3),
        target: NodeKey::from_u64(4),
        relation: RelationKind::Calls,
        span: facts.edges[0].span.clone(),
    });
    let path = temp_path("duplicates");
    write_repository_store(&path, &facts).unwrap();
    let decoded = decode_facts(&fs::read(&path).unwrap());
    assert_eq!(decoded.edges.len(), facts.edges.len());
    assert_eq!(decoded.canonicalized(), facts.canonicalized());
    cleanup(&[&path]);
}

struct FileRecord {
    path_id: u32,
    start: u64,
    count: u64,
}

fn file_record(bytes: &[u8], index: usize) -> FileRecord {
    let start = index * FILE_OWNERSHIP_RECORD_LEN as usize;
    let record = &bytes[start..start + FILE_OWNERSHIP_RECORD_LEN as usize];
    FileRecord {
        path_id: get_u32(record, OWNERSHIP_PATH_ID_OFFSET),
        start: get_u64(record, OWNERSHIP_CONTRIBUTION_START_OFFSET),
        count: get_u64(record, OWNERSHIP_CONTRIBUTION_COUNT_OFFSET),
    }
}

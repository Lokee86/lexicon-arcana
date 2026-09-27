use crate::repository::{NodeKind, RelationKind, UnresolvedReason};

use super::*;

fn valid_specs() -> [SectionSpec; SECTION_COUNT as usize] {
    [
        SectionSpec {
            byte_len: 48,
            record_count: 2,
            checksum: [1; 32],
        },
        SectionSpec {
            byte_len: NODE_RECORD_LEN * 2,
            record_count: 2,
            checksum: [2; 32],
        },
        SectionSpec {
            byte_len: EDGE_RECORD_LEN * 3,
            record_count: 3,
            checksum: [3; 32],
        },
        SectionSpec {
            byte_len: UNRESOLVED_RECORD_LEN,
            record_count: 1,
            checksum: [4; 32],
        },
        SectionSpec {
            byte_len: FILE_OWNERSHIP_RECORD_LEN + CONTRIBUTION_RECORD_LEN * 3,
            record_count: 1,
            checksum: [5; 32],
        },
        SectionSpec {
            byte_len: NAME_INDEX_RECORD_LEN * 2,
            record_count: 2,
            checksum: [6; 32],
        },
        SectionSpec {
            byte_len: PATH_INDEX_RECORD_LEN * 2,
            record_count: 2,
            checksum: [7; 32],
        },
        SectionSpec {
            byte_len: KIND_INDEX_RECORD_LEN * 2,
            record_count: 2,
            checksum: [8; 32],
        },
    ]
}

#[test]
fn frozen_layout_constants_match_v1_contract() {
    assert_eq!(MAGIC, *b"ARCREPO\0");
    assert_eq!(FORMAT_VERSION, 1);
    assert_eq!(HEADER_LEN, 512);
    assert_eq!(SECTION_COUNT, 8);
    assert_eq!(
        64 + usize::from(SECTION_COUNT) * 56,
        usize::from(HEADER_LEN)
    );
    assert_eq!(NODE_FLAGS_OFFSET + 2, NODE_RECORD_LEN as usize);
    assert_eq!(EDGE_RECORD_LEN, 40);
    assert_eq!(UNRESOLVED_RECORD_LEN, 56);
}

#[test]
fn stable_codes_cover_existing_semantics() {
    for kind in [
        NodeKind::Repository,
        NodeKind::Function,
        NodeKind::StatePath,
    ] {
        assert_eq!(node_kind_from_code(node_kind_code(&kind)), Some(kind));
    }
    for relation in [
        RelationKind::Contains,
        RelationKind::Calls,
        RelationKind::ConsumesMessage,
    ] {
        assert_eq!(relation_from_code(relation_code(&relation)), Some(relation));
    }
    for reason in [
        UnresolvedReason::MissingTarget,
        UnresolvedReason::MacroExpansionCycle,
        UnresolvedReason::CompilerIdentityMismatch,
    ] {
        assert_eq!(
            unresolved_reason_from_code(unresolved_reason_code(&reason)),
            Some(reason)
        );
    }
    assert_eq!(
        unresolved_reason_code(&UnresolvedReason::Unknown("future".into())),
        UNKNOWN_REASON_CODE
    );
}

#[test]
fn header_encoding_is_deterministic_and_round_trips() {
    let header = RepositoryHeader::layout(valid_specs(), [0x5a; 32]).unwrap();
    let first = header.encode();
    assert_eq!(first, header.encode());
    assert_eq!(&first[0..8], &MAGIC);
    assert_eq!(
        u16::from_le_bytes(first[8..10].try_into().unwrap()),
        FORMAT_VERSION
    );
    assert_eq!(RepositoryHeader::decode(&first).unwrap(), header);
}

#[test]
fn layout_is_aligned_ordered_and_exactly_bounded() {
    let header = RepositoryHeader::layout(valid_specs(), [0; 32]).unwrap();
    let mut previous_end = u64::from(HEADER_LEN);
    for kind in SectionKind::ALL {
        let section = header.section(kind);
        assert_eq!(section.offset % ALIGNMENT, 0);
        assert!(section.offset >= previous_end);
        previous_end = section.offset + section.byte_len;
    }
    assert_eq!(header.file_len % ALIGNMENT, 0);
}

#[test]
fn layout_rejects_size_overflow() {
    let mut specs = valid_specs();
    specs[SectionKind::Strings.index()].byte_len = u64::MAX;
    assert_eq!(
        RepositoryHeader::layout(specs, [0; 32]),
        Err(FormatError::SizeOverflow)
    );
}

#[test]
fn decode_rejects_out_of_bounds_and_misaligned_sections() {
    let header = RepositoryHeader::layout(valid_specs(), [0; 32]).unwrap();
    let mut bytes = header.encode();
    let edge_base = 64 + SectionKind::Edges.index() * 56;
    write_u64(&mut bytes, edge_base, header.file_len);
    assert_eq!(
        RepositoryHeader::decode(&bytes),
        Err(FormatError::SectionOutOfBounds(SectionKind::Edges))
    );

    let mut bytes = header.encode();
    let node_base = 64 + SectionKind::Nodes.index() * 56;
    write_u64(
        &mut bytes,
        node_base,
        header.section(SectionKind::Nodes).offset + 1,
    );
    assert_eq!(
        RepositoryHeader::decode(&bytes),
        Err(FormatError::MisalignedSection(SectionKind::Nodes))
    );
}

#[test]
fn fixed_width_sections_and_indexes_are_bounded() {
    let header = RepositoryHeader::layout(valid_specs(), [0; 32]).unwrap();
    let mut bytes = header.encode();
    let unresolved_base = 64 + SectionKind::Unresolved.index() * 56;
    write_u64(
        &mut bytes,
        unresolved_base + 8,
        header.section(SectionKind::Unresolved).byte_len + 1,
    );
    assert_eq!(
        RepositoryHeader::decode(&bytes),
        Err(FormatError::InvalidSectionLength(SectionKind::Unresolved))
    );

    let mut specs = valid_specs();
    specs[SectionKind::NameIndex.index()].record_count = 1;
    specs[SectionKind::NameIndex.index()].byte_len = NAME_INDEX_RECORD_LEN;
    assert_eq!(
        RepositoryHeader::layout(specs, [0; 32]),
        Err(FormatError::IndexCountMismatch(SectionKind::NameIndex))
    );
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

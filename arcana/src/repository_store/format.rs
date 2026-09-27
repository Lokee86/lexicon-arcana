//! Frozen on-disk layout for `repository.arcana` v1.
//!
//! All integers are little-endian. Sections are 8-byte aligned and occur in
//! `SectionKind::ALL` order. Every section has its own SHA-256 checksum; the
//! payload checksum covers all bytes after the fixed header, including zero
//! alignment padding.

use std::fmt;

#[path = "format_codes.rs"]
mod codes;
#[path = "format_header.rs"]
mod header;
#[path = "format_validation.rs"]
mod validation;

pub use codes::{
    node_kind_code, node_kind_from_code, relation_code, relation_from_code, unresolved_reason_code,
    unresolved_reason_from_code,
};

pub const MAGIC: [u8; 8] = *b"ARCREPO\0";
pub const FORMAT_VERSION: u16 = 1;
pub const HEADER_LEN: u16 = 512;
pub const SECTION_COUNT: u16 = 8;
pub const FLAGS: u16 = 0;
pub const ENDIAN_MARKER: u64 = 0x0102_0304_0506_0708;
pub const ALIGNMENT: u64 = 8;
pub const SHA256_LEN: usize = 32;
pub const ABSENT_STRING_ID: u32 = u32::MAX;
pub const UNKNOWN_REASON_CODE: u16 = u16::MAX;

pub const STRING_INDEX_RECORD_LEN: u64 = 16;
pub const NODE_RECORD_LEN: u64 = 88;
pub const EDGE_RECORD_LEN: u64 = 40;
pub const UNRESOLVED_RECORD_LEN: u64 = 56;
pub const FILE_OWNERSHIP_RECORD_LEN: u64 = 24;
pub const CONTRIBUTION_RECORD_LEN: u64 = 16;
pub const NAME_INDEX_RECORD_LEN: u64 = 4;
pub const PATH_INDEX_RECORD_LEN: u64 = 4;
pub const KIND_INDEX_RECORD_LEN: u64 = 8;

pub const NODE_FLAG_EXTERNAL_IDENTITY: u16 = 1 << 0;
pub const NODE_FLAG_CONTENT_ID: u16 = 1 << 1;
pub const NODE_FLAG_SPAN: u16 = 1 << 2;
pub const EDGE_FLAG_SPAN: u16 = 1 << 0;
pub const UNRESOLVED_FLAG_SPAN: u16 = 1 << 0;

/// String IDs are lexical UTF-8 order. The string section begins with one
/// 16-byte entry per string: blob-relative offset (u64), byte length (u32),
/// reserved zero (u32), followed by the concatenated UTF-8 byte blob.
pub const STRING_ENTRY_OFFSET_OFFSET: usize = 0;
pub const STRING_ENTRY_LENGTH_OFFSET: usize = 8;

/// Node records are unique by `NodeKey`, ordered by key, and row index equals
/// dense node ID. Exact duplicate node facts use a nonzero occurrence count.
/// External identities are raw 32-byte SHA-256 digests when the flag is set.
pub const NODE_KEY_OFFSET: usize = 0;
pub const NODE_EXTERNAL_IDENTITY_OFFSET: usize = 8;
pub const NODE_CONTENT_ID_OFFSET: usize = 40;
pub const NODE_PATH_ID_OFFSET: usize = 48;
pub const NODE_NAME_ID_OFFSET: usize = 52;
pub const NODE_QUALIFIED_NAME_ID_OFFSET: usize = 56;
pub const NODE_SPAN_PATH_ID_OFFSET: usize = 60;
pub const NODE_SPAN_START_LINE_OFFSET: usize = 64;
pub const NODE_SPAN_START_COLUMN_OFFSET: usize = 68;
pub const NODE_SPAN_END_LINE_OFFSET: usize = 72;
pub const NODE_SPAN_END_COLUMN_OFFSET: usize = 76;
pub const NODE_OCCURRENCE_COUNT_OFFSET: usize = 80;
pub const NODE_KIND_OFFSET: usize = 84;
pub const NODE_FLAGS_OFFSET: usize = 86;

/// Edge records preserve canonical `EdgeFact::Ord` occurrence facts. Graph
/// topology remains free to collapse equivalent source/target/relation edges.
pub const EDGE_SOURCE_OFFSET: usize = 0;
pub const EDGE_TARGET_OFFSET: usize = 8;
pub const EDGE_RELATION_OFFSET: usize = 16;
pub const EDGE_FLAGS_OFFSET: usize = 18;
pub const EDGE_SPAN_PATH_ID_OFFSET: usize = 20;
pub const EDGE_SPAN_START_LINE_OFFSET: usize = 24;
pub const EDGE_SPAN_START_COLUMN_OFFSET: usize = 28;
pub const EDGE_SPAN_END_LINE_OFFSET: usize = 32;
pub const EDGE_SPAN_END_COLUMN_OFFSET: usize = 36;

/// Unresolved records preserve canonical `UnresolvedReferenceFact::Ord`.
/// Unknown reasons use `UNKNOWN_REASON_CODE` plus a reason string ID.
pub const UNRESOLVED_SOURCE_OFFSET: usize = 0;
pub const UNRESOLVED_RELATION_OFFSET: usize = 8;
pub const UNRESOLVED_REASON_OFFSET: usize = 10;
pub const UNRESOLVED_FLAGS_OFFSET: usize = 12;
pub const UNRESOLVED_EXPRESSION_ID_OFFSET: usize = 16;
pub const UNRESOLVED_CANDIDATE_NAMESPACE_ID_OFFSET: usize = 20;
pub const UNRESOLVED_CANDIDATE_NAME_ID_OFFSET: usize = 24;
pub const UNRESOLVED_REASON_STRING_ID_OFFSET: usize = 28;
pub const UNRESOLVED_SPAN_PATH_ID_OFFSET: usize = 32;
pub const UNRESOLVED_SPAN_START_LINE_OFFSET: usize = 36;
pub const UNRESOLVED_SPAN_START_COLUMN_OFFSET: usize = 40;
pub const UNRESOLVED_SPAN_END_LINE_OFFSET: usize = 44;
pub const UNRESOLVED_SPAN_END_COLUMN_OFFSET: usize = 48;

/// Ownership begins with one 24-byte file record per owned path: path string
/// ID (u32), reserved zero (u32), contribution start (u64), contribution count
/// (u64). The tail is 16-byte contribution records: record index (u64), kind
/// tag (u8: node=1, edge=2, unresolved=3), seven reserved zero bytes.
pub const OWNERSHIP_PATH_ID_OFFSET: usize = 0;
pub const OWNERSHIP_CONTRIBUTION_START_OFFSET: usize = 8;
pub const OWNERSHIP_CONTRIBUTION_COUNT_OFFSET: usize = 16;
pub const CONTRIBUTION_RECORD_INDEX_OFFSET: usize = 0;
pub const CONTRIBUTION_KIND_OFFSET: usize = 8;

/// Name/path indexes are dense node IDs (u32), sorted by referenced UTF-8 bytes
/// then node ID. Kind index records are kind code (u16), reserved zero (u16),
/// dense node ID (u32), sorted by kind code then node ID.
pub const KIND_INDEX_KIND_OFFSET: usize = 0;
pub const KIND_INDEX_NODE_ID_OFFSET: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SectionKind {
    Strings = 0,
    Nodes = 1,
    Edges = 2,
    Unresolved = 3,
    Ownership = 4,
    NameIndex = 5,
    PathIndex = 6,
    KindIndex = 7,
}

impl SectionKind {
    pub const ALL: [Self; SECTION_COUNT as usize] = [
        Self::Strings,
        Self::Nodes,
        Self::Edges,
        Self::Unresolved,
        Self::Ownership,
        Self::NameIndex,
        Self::PathIndex,
        Self::KindIndex,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SectionSpec {
    pub byte_len: u64,
    pub record_count: u64,
    pub checksum: [u8; SHA256_LEN],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SectionDescriptor {
    pub offset: u64,
    pub byte_len: u64,
    pub record_count: u64,
    pub checksum: [u8; SHA256_LEN],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepositoryHeader {
    pub file_len: u64,
    pub payload_checksum: [u8; SHA256_LEN],
    pub sections: [SectionDescriptor; SECTION_COUNT as usize],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormatError {
    FileTooShort,
    InvalidMagic,
    UnsupportedVersion(u16),
    InvalidHeaderLength(u16),
    InvalidSectionCount(u16),
    UnsupportedFlags(u16),
    InvalidEndianMarker(u64),
    InvalidFileLength,
    SizeOverflow,
    MisalignedSection(SectionKind),
    SectionOutOfBounds(SectionKind),
    InvalidSectionLength(SectionKind),
    TooManyStrings,
    TooManyNodes,
    IndexCountMismatch(SectionKind),
}

impl fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid repository.arcana v1 layout: {self:?}")
    }
}

impl std::error::Error for FormatError {}

#[cfg(test)]
#[path = "format_tests.rs"]
mod tests;

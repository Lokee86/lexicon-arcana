use std::fs::{self, OpenOptions};
use std::path::Path;

use crate::repository::RepositoryFacts;
use crate::storage::StableHasher;

use super::canonical::CanonicalFacts;
use super::format::{RepositoryHeader, SECTION_COUNT, SectionKind, SectionSpec};
use super::writer_sections::{
    write_edges, write_kind_index, write_name_index, write_nodes, write_ownership,
    write_path_index, write_strings, write_unresolved,
};
use super::writer_sink::PayloadWriter;
use super::{BorrowedStringTable, RepositoryStoreWriteError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepositoryStoreWrite {
    pub header: RepositoryHeader,
    pub node_key_checksum: u64,
}

pub fn write_repository_store(
    path: impl AsRef<Path>,
    facts: &RepositoryFacts,
) -> Result<RepositoryStoreWrite, RepositoryStoreWriteError> {
    let path = path.as_ref();
    let canonical = CanonicalFacts::prepare(facts)?;
    canonical.node_count()?;

    let strings =
        BorrowedStringTable::collect(facts, canonical.ownership.keys().map(String::as_str))?;

    let node_key_checksum = node_key_checksum(&canonical);
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let result = write_open(file, &canonical, &strings);
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result.map(|header| RepositoryStoreWrite {
        header,
        node_key_checksum,
    })
}

fn node_key_checksum(canonical: &CanonicalFacts<'_>) -> u64 {
    let mut hasher = StableHasher::new();
    for node in &canonical.nodes {
        hasher.update(&node.fact.key.0.to_le_bytes());
    }
    hasher.finish()
}

fn write_open(
    file: std::fs::File,
    canonical: &CanonicalFacts<'_>,
    strings: &BorrowedStringTable<'_>,
) -> Result<RepositoryHeader, RepositoryStoreWriteError> {
    let mut writer = PayloadWriter::new(file)?;
    let mut specs = [SectionSpec::default(); SECTION_COUNT as usize];

    specs[SectionKind::Strings.index()] =
        writer.section(as_u64(strings.len())?, |sink| write_strings(sink, strings))?;
    specs[SectionKind::Nodes.index()] = writer.section(as_u64(canonical.nodes.len())?, |sink| {
        write_nodes(sink, canonical, strings)
    })?;
    specs[SectionKind::Edges.index()] = writer.section(as_u64(canonical.edges.len())?, |sink| {
        write_edges(sink, canonical, strings)
    })?;
    specs[SectionKind::Unresolved.index()] = writer
        .section(as_u64(canonical.unresolved.len())?, |sink| {
            write_unresolved(sink, canonical, strings)
        })?;
    specs[SectionKind::Ownership.index()] = writer
        .section(as_u64(canonical.ownership.len())?, |sink| {
            write_ownership(sink, canonical, strings)
        })?;
    specs[SectionKind::NameIndex.index()] = writer
        .section(as_u64(canonical.nodes.len())?, |sink| {
            write_name_index(sink, canonical)
        })?;
    specs[SectionKind::PathIndex.index()] = writer
        .section(as_u64(canonical.nodes.len())?, |sink| {
            write_path_index(sink, canonical)
        })?;
    specs[SectionKind::KindIndex.index()] = writer
        .section(as_u64(canonical.nodes.len())?, |sink| {
            write_kind_index(sink, canonical)
        })?;

    writer.finish(specs)
}

fn as_u64(value: usize) -> Result<u64, RepositoryStoreWriteError> {
    u64::try_from(value).map_err(|_| RepositoryStoreWriteError::SizeOverflow)
}

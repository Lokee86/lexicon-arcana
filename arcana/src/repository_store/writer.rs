use std::fs::{self, OpenOptions};
use std::path::Path;

use crate::repository::RepositoryFacts;
use crate::storage::StableHasher;

use super::RepositoryStoreWriteError;
use super::build::CompactRepositoryBuild;
use super::format::{RepositoryHeader, SECTION_COUNT, SectionKind, SectionSpec};
use super::writer_sections::{
    write_edges, write_kind_index, write_name_index, write_nodes, write_ownership,
    write_path_index, write_strings, write_unresolved,
};
use super::writer_sink::PayloadWriter;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepositoryStoreWrite {
    pub header: RepositoryHeader,
    pub node_key_checksum: u64,
}

pub fn write_repository_store(
    path: impl AsRef<Path>,
    facts: &RepositoryFacts,
) -> Result<RepositoryStoreWrite, RepositoryStoreWriteError> {
    let build = CompactRepositoryBuild::from_facts(facts)?;
    write_repository_store_compact(path, &build)
}

pub(crate) fn write_repository_store_compact(
    path: impl AsRef<Path>,
    build: &CompactRepositoryBuild,
) -> Result<RepositoryStoreWrite, RepositoryStoreWriteError> {
    let path = path.as_ref();
    let node_key_checksum = node_key_checksum(build);
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let result = write_open(file, build);
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result.map(|header| RepositoryStoreWrite {
        header,
        node_key_checksum,
    })
}

fn node_key_checksum(build: &CompactRepositoryBuild) -> u64 {
    let mut hasher = StableHasher::new();
    for node in &build.nodes {
        hasher.update(&node.key.0.to_le_bytes());
    }
    hasher.finish()
}

fn write_open(
    file: std::fs::File,
    build: &CompactRepositoryBuild,
) -> Result<RepositoryHeader, RepositoryStoreWriteError> {
    let mut writer = PayloadWriter::new(file)?;
    let mut specs = [SectionSpec::default(); SECTION_COUNT as usize];

    specs[SectionKind::Strings.index()] = writer.section(as_u64(build.strings.len())?, |sink| {
        write_strings(sink, &build.strings)
    })?;
    specs[SectionKind::Nodes.index()] =
        writer.section(as_u64(build.nodes.len())?, |sink| write_nodes(sink, build))?;
    specs[SectionKind::Edges.index()] =
        writer.section(as_u64(build.edges.len())?, |sink| write_edges(sink, build))?;
    specs[SectionKind::Unresolved.index()] = writer
        .section(as_u64(build.unresolved.len())?, |sink| {
            write_unresolved(sink, build)
        })?;
    specs[SectionKind::Ownership.index()] = writer
        .section(as_u64(build.ownership.len())?, |sink| {
            write_ownership(sink, build)
        })?;
    specs[SectionKind::NameIndex.index()] = writer
        .section(as_u64(build.name_index.len())?, |sink| {
            write_name_index(sink, build)
        })?;
    specs[SectionKind::PathIndex.index()] = writer
        .section(as_u64(build.path_index.len())?, |sink| {
            write_path_index(sink, build)
        })?;
    specs[SectionKind::KindIndex.index()] = writer
        .section(as_u64(build.kind_index.len())?, |sink| {
            write_kind_index(sink, build)
        })?;

    writer.finish(specs)
}

fn as_u64(value: usize) -> Result<u64, RepositoryStoreWriteError> {
    u64::try_from(value).map_err(|_| RepositoryStoreWriteError::SizeOverflow)
}

use std::fs::{self, OpenOptions};
use std::path::Path;

use super::format::{SECTION_COUNT, SectionKind, SectionSpec};
use super::rewrite_indexes::IndexStage;
use super::rewrite_io::copy_file;
use super::rewrite_ownership::OwnershipStage;
use super::rewrite_records::{NodeStage, RecordStage};
use super::rewrite_strings::RewriteStrings;
use super::writer_sink::PayloadWriter;
use super::{RepositoryStoreWrite, RepositoryStoreWriteError};

pub(super) fn write_rewritten_store(
    path: &Path,
    strings: &RewriteStrings,
    nodes: &NodeStage,
    edges: &RecordStage,
    unresolved: &RecordStage,
    ownership: &OwnershipStage,
    indexes: &IndexStage,
) -> Result<RepositoryStoreWrite, RepositoryStoreWriteError> {
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let result = write_open(file, strings, nodes, edges, unresolved, ownership, indexes);
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result.map(|header| RepositoryStoreWrite {
        header,
        node_key_checksum: nodes.node_key_checksum,
    })
}

fn write_open(
    file: std::fs::File,
    strings: &RewriteStrings,
    nodes: &NodeStage,
    edges: &RecordStage,
    unresolved: &RecordStage,
    ownership: &OwnershipStage,
    indexes: &IndexStage,
) -> Result<super::format::RepositoryHeader, RepositoryStoreWriteError> {
    let mut writer = PayloadWriter::new(file)?;
    let mut specs = [SectionSpec::default(); SECTION_COUNT as usize];

    specs[SectionKind::Strings.index()] =
        writer.section(strings.count, |sink| strings.write_to(sink))?;
    specs[SectionKind::Nodes.index()] =
        writer.section(nodes.count, |sink| copy_file(&nodes.path, sink))?;
    specs[SectionKind::Edges.index()] =
        writer.section(edges.count, |sink| copy_file(&edges.path, sink))?;
    specs[SectionKind::Unresolved.index()] =
        writer.section(unresolved.count, |sink| copy_file(&unresolved.path, sink))?;
    specs[SectionKind::Ownership.index()] = writer.section(ownership.count, |sink| {
        copy_file(&ownership.records_path, sink)?;
        copy_file(&ownership.contributions_path, sink)
    })?;
    specs[SectionKind::NameIndex.index()] =
        writer.section(indexes.count, |sink| copy_file(&indexes.name_path, sink))?;
    specs[SectionKind::PathIndex.index()] =
        writer.section(indexes.count, |sink| copy_file(&indexes.path_path, sink))?;
    specs[SectionKind::KindIndex.index()] =
        writer.section(indexes.count, |sink| copy_file(&indexes.kind_path, sink))?;

    writer.finish(specs)
}

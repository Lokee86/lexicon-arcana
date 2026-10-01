use std::collections::BTreeSet;

use super::format::{EDGE_RECORD_LEN, UNRESOLVED_RECORD_LEN};
use super::rewrite_io::FixedSectionReader;
use super::rewrite_strings::RewriteStrings;
use super::{
    CompactEdgeRecord, CompactNodeRecord, CompactRepositoryDelta, CompactSpan,
    CompactUnresolvedRecord, RepositoryStoreWriteError,
};

pub(super) fn next_base_edge(
    reader: &mut FixedSectionReader<{ EDGE_RECORD_LEN as usize }>,
    changed: &BTreeSet<u64>,
    strings: &mut RewriteStrings,
) -> Result<Option<(u64, CompactEdgeRecord)>, RepositoryStoreWriteError> {
    while let Some((index, bytes)) = reader.next()? {
        if changed.contains(&index) {
            continue;
        }
        return Ok(Some((
            index,
            remap_base_edge(CompactEdgeRecord::decode(&bytes)?, strings)?,
        )));
    }
    Ok(None)
}

pub(super) fn next_delta_edge(
    delta: &CompactRepositoryDelta,
    index: usize,
    strings: &RewriteStrings,
) -> Result<Option<CompactEdgeRecord>, RepositoryStoreWriteError> {
    delta
        .edges()
        .get(index)
        .copied()
        .map(|record| remap_delta_edge(record, strings))
        .transpose()
}

pub(super) fn next_base_unresolved(
    reader: &mut FixedSectionReader<{ UNRESOLVED_RECORD_LEN as usize }>,
    changed: &BTreeSet<u64>,
    strings: &mut RewriteStrings,
) -> Result<Option<(u64, CompactUnresolvedRecord)>, RepositoryStoreWriteError> {
    while let Some((index, bytes)) = reader.next()? {
        if changed.contains(&index) {
            continue;
        }
        return Ok(Some((
            index,
            remap_base_unresolved(CompactUnresolvedRecord::decode(&bytes)?, strings)?,
        )));
    }
    Ok(None)
}

pub(super) fn next_delta_unresolved(
    delta: &CompactRepositoryDelta,
    index: usize,
    strings: &RewriteStrings,
) -> Result<Option<CompactUnresolvedRecord>, RepositoryStoreWriteError> {
    delta
        .unresolved()
        .get(index)
        .copied()
        .map(|record| remap_delta_unresolved(record, strings))
        .transpose()
}

pub(super) fn remap_base_node(
    mut record: CompactNodeRecord,
    strings: &mut RewriteStrings,
) -> Result<CompactNodeRecord, RepositoryStoreWriteError> {
    record.path = strings.remap_base(record.path)?;
    record.name = strings.remap_base(record.name)?;
    record.qualified_name = strings.remap_base(record.qualified_name)?;
    record.span = record
        .span
        .map(|span| remap_base_span(span, strings))
        .transpose()?;
    Ok(record)
}

pub(super) fn remap_delta_node(
    mut record: CompactNodeRecord,
    strings: &RewriteStrings,
) -> Result<CompactNodeRecord, RepositoryStoreWriteError> {
    record.path = strings.remap_delta(record.path)?;
    record.name = strings.remap_delta(record.name)?;
    record.qualified_name = strings.remap_delta(record.qualified_name)?;
    record.span = record
        .span
        .map(|span| remap_delta_span(span, strings))
        .transpose()?;
    Ok(record)
}

fn remap_base_edge(
    mut record: CompactEdgeRecord,
    strings: &mut RewriteStrings,
) -> Result<CompactEdgeRecord, RepositoryStoreWriteError> {
    record.span = record
        .span
        .map(|span| remap_base_span(span, strings))
        .transpose()?;
    Ok(record)
}

fn remap_delta_edge(
    mut record: CompactEdgeRecord,
    strings: &RewriteStrings,
) -> Result<CompactEdgeRecord, RepositoryStoreWriteError> {
    record.span = record
        .span
        .map(|span| remap_delta_span(span, strings))
        .transpose()?;
    Ok(record)
}

fn remap_base_unresolved(
    mut record: CompactUnresolvedRecord,
    strings: &mut RewriteStrings,
) -> Result<CompactUnresolvedRecord, RepositoryStoreWriteError> {
    record.expression = strings.remap_base(record.expression)?;
    record.candidate_namespace = strings.remap_base(record.candidate_namespace)?;
    record.candidate_name = strings.remap_base(record.candidate_name)?;
    record.unknown_reason = strings.remap_base(record.unknown_reason)?;
    record.span = record
        .span
        .map(|span| remap_base_span(span, strings))
        .transpose()?;
    Ok(record)
}

fn remap_delta_unresolved(
    mut record: CompactUnresolvedRecord,
    strings: &RewriteStrings,
) -> Result<CompactUnresolvedRecord, RepositoryStoreWriteError> {
    record.expression = strings.remap_delta(record.expression)?;
    record.candidate_namespace = strings.remap_delta(record.candidate_namespace)?;
    record.candidate_name = strings.remap_delta(record.candidate_name)?;
    record.unknown_reason = strings.remap_delta(record.unknown_reason)?;
    record.span = record
        .span
        .map(|span| remap_delta_span(span, strings))
        .transpose()?;
    Ok(record)
}

fn remap_base_span(
    mut span: CompactSpan,
    strings: &mut RewriteStrings,
) -> Result<CompactSpan, RepositoryStoreWriteError> {
    span.path = strings.remap_base(span.path)?;
    Ok(span)
}

fn remap_delta_span(
    mut span: CompactSpan,
    strings: &RewriteStrings,
) -> Result<CompactSpan, RepositoryStoreWriteError> {
    span.path = strings.remap_delta(span.path)?;
    Ok(span)
}

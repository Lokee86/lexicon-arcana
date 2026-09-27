use super::LexiconSnapshotError;
use super::binary_v2_reader::SpanRef;
use super::binary_v2_stream::NodeRef;
use super::identity::LexiconIdentity;
use crate::repository_store::{CompactRepositoryAssembler, TempStringId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NodeSignature {
    attributes: Option<Box<[u8]>>,
    content_id: Option<LexiconIdentity>,
    kind: TempStringId,
    name: TempStringId,
    owner: Option<TempStringId>,
    path: TempStringId,
    qualified_name: TempStringId,
    span: Option<RawSpan>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawSpan {
    path: TempStringId,
    start_line: u64,
    start_column: u64,
    end_line: u64,
    end_column: u64,
}

pub(super) fn signature(
    assembler: &mut CompactRepositoryAssembler,
    record: &NodeRef<'_>,
) -> Result<NodeSignature, LexiconSnapshotError> {
    Ok(NodeSignature {
        attributes: record
            .attributes
            .map(<[u8]>::to_vec)
            .map(Vec::into_boxed_slice),
        content_id: record.content_id,
        kind: assembler.intern(record.kind)?,
        name: assembler.intern(record.name)?,
        owner: optional_intern(assembler, record.owner)?,
        path: assembler.intern(record.path)?,
        qualified_name: assembler.intern(record.qualified_name)?,
        span: raw_span(assembler, record.span)?,
    })
}

pub(super) fn optional_intern(
    assembler: &mut CompactRepositoryAssembler,
    value: Option<&str>,
) -> Result<Option<TempStringId>, LexiconSnapshotError> {
    value
        .map(|value| assembler.intern(value))
        .transpose()
        .map_err(Into::into)
}

fn raw_span(
    assembler: &mut CompactRepositoryAssembler,
    span: Option<SpanRef<'_>>,
) -> Result<Option<RawSpan>, LexiconSnapshotError> {
    span.map(|span| {
        Ok(RawSpan {
            path: assembler.intern(span.path)?,
            start_line: span.start_line,
            start_column: span.start_column,
            end_line: span.end_line,
            end_column: span.end_column,
        })
    })
    .transpose()
}

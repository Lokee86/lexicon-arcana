use super::LexiconSnapshotError;
use super::binary_v2_reader::SpanRef;
use super::binary_v2_stream::{EdgeRef, NodeRef, UnresolvedRef};
use super::object::{EdgeRecord, NodeRecord, SpanRecord, UnresolvedRecord};
use super::stream_compact::CompactPass;
use crate::repository_store::RepositoryStoreFile;

pub(super) fn ingest_node(
    pass: &mut CompactPass,
    record: NodeRecord,
) -> Result<(), LexiconSnapshotError> {
    let span = record.span.as_ref().map(span_ref);
    pass.ingest_node(NodeRef {
        attributes: record.attributes.as_deref(),
        content_id: record.content_id,
        id: record.id,
        kind: &record.kind,
        name: &record.name,
        owner: record.owner.as_deref(),
        path: &record.path,
        qualified_name: &record.qualified_name,
        span,
    })
}

pub(super) fn ingest_edge(
    pass: &mut CompactPass,
    record: EdgeRecord,
) -> Result<(), LexiconSnapshotError> {
    let span = record.span.as_ref().map(span_ref);
    pass.ingest_edge(EdgeRef {
        owner: record.owner.as_deref(),
        relation: &record.relation,
        source: record.source,
        span,
        target: record.target,
    })
}

pub(super) fn ingest_edge_with_base(
    pass: &mut CompactPass,
    record: EdgeRecord,
    base: &mut RepositoryStoreFile,
) -> Result<(), LexiconSnapshotError> {
    let span = record.span.as_ref().map(span_ref);
    pass.ingest_edge_with_base(
        EdgeRef {
            owner: record.owner.as_deref(),
            relation: &record.relation,
            source: record.source,
            span,
            target: record.target,
        },
        base,
    )
}

pub(super) fn ingest_unresolved(
    pass: &mut CompactPass,
    record: UnresolvedRecord,
) -> Result<(), LexiconSnapshotError> {
    let span = record.span.as_ref().map(span_ref);
    pass.ingest_unresolved(UnresolvedRef {
        candidate_name: record.candidate_name.as_deref(),
        candidate_namespace: record.candidate_namespace.as_deref(),
        expression: &record.expression,
        owner: record.owner.as_deref(),
        reason: &record.reason,
        relation: &record.relation,
        source: record.source,
        span,
    })
}

pub(super) fn ingest_unresolved_with_base(
    pass: &mut CompactPass,
    record: UnresolvedRecord,
    base: &mut RepositoryStoreFile,
) -> Result<(), LexiconSnapshotError> {
    let span = record.span.as_ref().map(span_ref);
    pass.ingest_unresolved_with_base(
        UnresolvedRef {
            candidate_name: record.candidate_name.as_deref(),
            candidate_namespace: record.candidate_namespace.as_deref(),
            expression: &record.expression,
            owner: record.owner.as_deref(),
            reason: &record.reason,
            relation: &record.relation,
            source: record.source,
            span,
        },
        base,
    )
}

fn span_ref(span: &SpanRecord) -> SpanRef<'_> {
    SpanRef {
        path: &span.path,
        start_line: span.start_line,
        start_column: span.start_column,
        end_line: span.end_line,
        end_column: span.end_column,
    }
}

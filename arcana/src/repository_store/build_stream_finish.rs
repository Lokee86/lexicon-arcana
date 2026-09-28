use super::build::CompactRepositoryBuild;
use super::build_indexes::{canonicalize_edges, canonicalize_unresolved};
use super::build_stream::{
    CompactRepositoryAssembler, TempEdgeRecord, TempNodeRecord, TempSpan, TempUnresolvedRecord,
};
use super::{
    CompactEdgeRecord, CompactNodeRecord, CompactSpan, CompactUnresolvedRecord,
    RepositoryStoreWriteError, StringId, TempStringId,
};

pub(super) fn finish_stream_build(
    build: CompactRepositoryAssembler,
) -> Result<CompactRepositoryBuild, RepositoryStoreWriteError> {
    let CompactRepositoryAssembler {
        strings: staged_strings,
        nodes: staged_nodes,
        edges: staged_edges,
        unresolved: staged_unresolved,
    } = build;

    let used = used_strings(
        staged_strings.len(),
        &staged_nodes,
        &staged_edges,
        &staged_unresolved,
    );
    let (strings, remap) = staged_strings.freeze(&used)?;

    let nodes = finish_nodes(staged_nodes, &remap);
    let edges = finish_edges(staged_edges, &remap);
    let unresolved = finish_unresolved(staged_unresolved, &remap);

    CompactRepositoryBuild::from_canonical_records(strings, nodes, edges, unresolved)
}

fn finish_nodes(records: Vec<TempNodeRecord>, remap: &[StringId]) -> Vec<CompactNodeRecord> {
    let mut nodes = Vec::with_capacity(records.len());
    nodes.extend(records.into_iter().map(|record| remap_node(record, remap)));
    nodes.sort_unstable_by_key(|record| record.key);
    nodes
}

fn finish_edges(records: Vec<TempEdgeRecord>, remap: &[StringId]) -> Vec<CompactEdgeRecord> {
    let mut edges = Vec::with_capacity(records.len());
    edges.extend(records.into_iter().map(|record| remap_edge(record, remap)));
    canonicalize_edges(&mut edges);
    edges
}

fn finish_unresolved(
    records: Vec<TempUnresolvedRecord>,
    remap: &[StringId],
) -> Vec<CompactUnresolvedRecord> {
    let mut unresolved = Vec::with_capacity(records.len());
    unresolved.extend(
        records
            .into_iter()
            .map(|record| remap_unresolved(record, remap)),
    );
    canonicalize_unresolved(&mut unresolved);
    unresolved
}

fn used_strings(
    string_count: usize,
    nodes: &[TempNodeRecord],
    edges: &[TempEdgeRecord],
    unresolved: &[TempUnresolvedRecord],
) -> Vec<bool> {
    let mut used = vec![false; string_count];
    let mut mark = |id: TempStringId| used[id.0 as usize] = true;
    for node in nodes {
        for id in [node.path, node.name, node.qualified_name] {
            mark(id);
        }
        if let Some(span) = node.span {
            mark(span.path);
        }
    }
    for edge in edges {
        if let Some(span) = edge.span {
            mark(span.path);
        }
    }
    for record in unresolved {
        mark(record.expression);
        for id in [
            record.candidate_namespace,
            record.candidate_name,
            record.unknown_reason,
        ]
        .into_iter()
        .flatten()
        {
            mark(id);
        }
        if let Some(span) = record.span {
            mark(span.path);
        }
    }
    used
}

fn remap_node(record: TempNodeRecord, remap: &[StringId]) -> CompactNodeRecord {
    CompactNodeRecord {
        key: record.key,
        external_identity: Some(record.external_identity),
        content_id: record.content_id,
        path: id(record.path, remap),
        name: id(record.name, remap),
        qualified_name: id(record.qualified_name, remap),
        span: record.span.map(|span| remap_span(span, remap)),
        occurrence_count: 1,
        kind_code: record.kind_code,
    }
}

fn remap_edge(record: TempEdgeRecord, remap: &[StringId]) -> CompactEdgeRecord {
    CompactEdgeRecord {
        source: record.source,
        target: record.target,
        relation_code: record.relation_code,
        span: record.span.map(|span| remap_span(span, remap)),
    }
}

fn remap_unresolved(record: TempUnresolvedRecord, remap: &[StringId]) -> CompactUnresolvedRecord {
    CompactUnresolvedRecord {
        source: record.source,
        relation_code: record.relation_code,
        reason_code: record.reason_code,
        expression: id(record.expression, remap),
        candidate_namespace: optional_id(record.candidate_namespace, remap),
        candidate_name: optional_id(record.candidate_name, remap),
        unknown_reason: optional_id(record.unknown_reason, remap),
        span: record.span.map(|span| remap_span(span, remap)),
    }
}

fn remap_span(span: TempSpan, remap: &[StringId]) -> CompactSpan {
    CompactSpan {
        path: id(span.path, remap),
        start_line: span.start_line,
        start_column: span.start_column,
        end_line: span.end_line,
        end_column: span.end_column,
    }
}

fn id(value: TempStringId, remap: &[StringId]) -> StringId {
    remap[value.0 as usize]
}

fn optional_id(value: Option<TempStringId>, remap: &[StringId]) -> StringId {
    StringId::optional(value.map(|value| id(value, remap)))
}

#[cfg(test)]
#[path = "build_stream_finish_tests.rs"]
mod tests;

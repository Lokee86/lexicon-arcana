use super::build::CompactRepositoryBuild;
use super::build_indexes::{canonicalize_edges, canonicalize_unresolved};
use super::build_stream::{
    CompactRepositoryAssembler, TempEdgeRecord, TempNodeRecord, TempSpan, TempStringId,
    TempUnresolvedRecord,
};
use super::{
    CompactEdgeRecord, CompactNodeRecord, CompactSpan, CompactStringTable, CompactUnresolvedRecord,
    RepositoryStoreWriteError, StringId,
};

pub(super) fn finish_stream_build(
    mut build: CompactRepositoryAssembler,
) -> Result<CompactRepositoryBuild, RepositoryStoreWriteError> {
    let used = used_strings(&build);
    let (strings, remap) = canonical_strings(build.strings, &used)?;

    let mut nodes = build
        .nodes
        .drain(..)
        .map(|record| remap_node(record, &remap))
        .collect::<Vec<_>>();
    nodes.sort_unstable_by_key(|record| record.key);

    let mut edges = build
        .edges
        .drain(..)
        .map(|record| remap_edge(record, &remap))
        .collect::<Vec<_>>();
    canonicalize_edges(&mut edges);

    let mut unresolved = build
        .unresolved
        .drain(..)
        .map(|record| remap_unresolved(record, &remap))
        .collect::<Vec<_>>();
    canonicalize_unresolved(&mut unresolved);

    CompactRepositoryBuild::from_canonical_records(strings, nodes, edges, unresolved)
}

fn used_strings(build: &CompactRepositoryAssembler) -> Vec<bool> {
    let mut used = vec![false; build.strings.len()];
    let mut mark = |id: TempStringId| used[id.0 as usize] = true;
    for node in &build.nodes {
        for id in [node.path, node.name, node.qualified_name] {
            mark(id);
        }
        if let Some(span) = node.span {
            mark(span.path);
        }
    }
    for edge in &build.edges {
        if let Some(span) = edge.span {
            mark(span.path);
        }
    }
    for record in &build.unresolved {
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

fn canonical_strings(
    strings: std::collections::BTreeMap<String, TempStringId>,
    used: &[bool],
) -> Result<(CompactStringTable, Vec<StringId>), RepositoryStoreWriteError> {
    let mut remap = vec![StringId::ABSENT; used.len()];
    let mut values = Vec::new();
    for (value, old) in strings {
        if !used[old.0 as usize] {
            continue;
        }
        let new = u32::try_from(values.len())
            .map(StringId)
            .map_err(|_| super::StoreFormatError::TooManyStrings)?;
        remap[old.0 as usize] = new;
        values.push(value);
    }
    Ok((CompactStringTable::from_sorted(values)?, remap))
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

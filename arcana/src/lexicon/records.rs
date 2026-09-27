use std::collections::BTreeMap;

use super::LexiconSnapshotError;
#[cfg(test)]
use super::object::FactRecord;
use super::object::{EdgeRecord, NodeRecord, SpanRecord, UnresolvedRecord};
use crate::repository::{
    ContentId, EdgeFact, NodeFact, NodeKey, NodeKind, RelationKind, RepositoryFacts, SourceSpan,
    UnresolvedReason, UnresolvedReferenceFact, normalize_repository_path,
};

pub(super) type CompatibilityCounts = BTreeMap<String, usize>;
pub(super) type ExternalNodeIds = BTreeMap<String, NodeKey>;
pub(super) type CompactNodeIds = BTreeMap<NodeKey, String>;

#[cfg(test)]
pub(super) fn build_repository_facts(
    records: Vec<FactRecord>,
) -> Result<(RepositoryFacts, Vec<String>), LexiconSnapshotError> {
    let mut nodes = BTreeMap::<String, NodeRecord>::new();
    let mut edges = Vec::new();
    let mut unresolved = Vec::new();
    for record in records {
        match record {
            FactRecord::Node(record) => insert_node_record(&mut nodes, record)?,
            FactRecord::Edge(record) => edges.push(record),
            FactRecord::Unresolved(record) => unresolved.push(record),
        }
    }

    let mut facts = RepositoryFacts::default();
    let mut external_ids = ExternalNodeIds::new();
    let mut compact_ids = CompactNodeIds::new();
    let mut compatibility = CompatibilityCounts::new();
    for record in nodes.into_values() {
        facts.nodes.push(convert_node(
            record,
            &mut external_ids,
            &mut compact_ids,
            &mut compatibility,
        )?);
    }
    for record in edges {
        if let Some(edge) = convert_edge(&external_ids, record, &mut compatibility)? {
            facts.edges.push(edge);
        }
    }
    for record in unresolved {
        if let Some(reference) = convert_unresolved(&external_ids, record, &mut compatibility)? {
            facts.unresolved.push(reference);
        }
    }
    finish_repository_facts(facts, compatibility)
}

pub(super) fn insert_node_record(
    nodes: &mut BTreeMap<String, NodeRecord>,
    record: NodeRecord,
) -> Result<(), LexiconSnapshotError> {
    match nodes.get(&record.id) {
        Some(existing) if existing != &record => {
            Err(LexiconSnapshotError::ConflictingNode(record.id))
        }
        Some(_) => Ok(()),
        None => {
            nodes.insert(record.id.clone(), record);
            Ok(())
        }
    }
}

pub(super) fn convert_node(
    record: NodeRecord,
    external_ids: &mut ExternalNodeIds,
    compact_ids: &mut CompactNodeIds,
    compatibility: &mut CompatibilityCounts,
) -> Result<NodeFact, LexiconSnapshotError> {
    validate_sha256_id(&record.id)?;
    validate_owner(record.owner.as_deref())?;
    let path = normalize_path(&record.path)?;
    let key = NodeKey::from_identity(record.id.as_bytes());
    if compact_ids
        .insert(key, record.id.clone())
        .is_some_and(|existing| existing != record.id)
    {
        return Err(LexiconSnapshotError::Malformed("node identity collision"));
    }
    external_ids.insert(record.id.clone(), key);
    let content_id = record
        .content_id
        .as_deref()
        .map(|id| -> Result<ContentId, LexiconSnapshotError> {
            validate_sha256_id(id)?;
            Ok(ContentId::from_bytes(id.as_bytes()))
        })
        .transpose()?;
    let kind = NodeKind::parse(&record.kind).unwrap_or_else(|| {
        *compatibility
            .entry(format!(
                "unrecognized Lexicon node kind {:?}; treating as symbol",
                record.kind
            ))
            .or_default() += 1;
        NodeKind::Symbol
    });
    if record.qualified_name.is_empty() {
        return Err(LexiconSnapshotError::Malformed("node qualified name"));
    }
    Ok(NodeFact {
        key,
        external_identity: Some(record.id),
        kind,
        path,
        name: record.name,
        qualified_name: record.qualified_name,
        content_id,
        span: convert_span(record.span)?,
    })
}

pub(super) fn convert_edge(
    ids: &ExternalNodeIds,
    record: EdgeRecord,
    compatibility: &mut CompatibilityCounts,
) -> Result<Option<EdgeFact>, LexiconSnapshotError> {
    validate_owner(record.owner.as_deref())?;
    let source = lookup_id(ids, &record.source)?;
    let target = lookup_id(ids, &record.target)?;
    let Some(relation) = RelationKind::parse(&record.relation) else {
        *compatibility
            .entry(format!(
                "unrecognized Lexicon edge relation {:?}; skipping edge",
                record.relation
            ))
            .or_default() += 1;
        return Ok(None);
    };
    Ok(Some(EdgeFact {
        source,
        target,
        relation,
        span: convert_span(record.span)?,
    }))
}

pub(super) fn convert_unresolved(
    ids: &ExternalNodeIds,
    record: UnresolvedRecord,
    compatibility: &mut CompatibilityCounts,
) -> Result<Option<UnresolvedReferenceFact>, LexiconSnapshotError> {
    validate_owner(record.owner.as_deref())?;
    let source = lookup_id(ids, &record.source)?;
    let Some(relation) = RelationKind::parse(&record.relation) else {
        *compatibility
            .entry(format!(
                "unrecognized Lexicon unresolved relation {:?}; skipping record",
                record.relation
            ))
            .or_default() += 1;
        return Ok(None);
    };
    let reason = UnresolvedReason::parse(&record.reason)
        .ok_or(LexiconSnapshotError::Malformed("empty unresolved reason"))?;
    if reason.is_unknown() {
        *compatibility
            .entry(format!(
                "unrecognized Lexicon unresolved reason {:?}; preserving label",
                record.reason
            ))
            .or_default() += 1;
    }
    Ok(Some(UnresolvedReferenceFact {
        source,
        relation,
        expression: record.expression,
        candidate_namespace: record.candidate_namespace,
        candidate_name: record.candidate_name,
        reason,
        span: convert_span(record.span)?,
    }))
}

pub(super) fn finish_repository_facts(
    mut facts: RepositoryFacts,
    compatibility: CompatibilityCounts,
) -> Result<(RepositoryFacts, Vec<String>), LexiconSnapshotError> {
    facts.nodes.sort_unstable();
    facts.nodes.dedup();
    facts.edges.sort_unstable();
    facts.edges.dedup();
    facts.unresolved.sort_unstable();
    facts.unresolved.dedup();
    let warnings = compatibility
        .into_iter()
        .map(|(message, count)| format!("{message} ({count} record(s))"))
        .collect();
    Ok((facts, warnings))
}

fn lookup_id(ids: &ExternalNodeIds, external_id: &str) -> Result<NodeKey, LexiconSnapshotError> {
    validate_sha256_id(external_id)?;
    ids.get(external_id)
        .copied()
        .ok_or(LexiconSnapshotError::Malformed("unknown relationship node"))
}

fn validate_owner(owner: Option<&str>) -> Result<(), LexiconSnapshotError> {
    if let Some(owner) = owner {
        normalize_path(owner)?;
    }
    Ok(())
}

fn convert_span(span: Option<SpanRecord>) -> Result<Option<SourceSpan>, LexiconSnapshotError> {
    let Some(span) = span else {
        return Ok(None);
    };
    Ok(Some(SourceSpan {
        path: normalize_path(&span.path)?,
        start_line: u32_value(span.start_line, "span start line")?,
        start_column: u32_value(span.start_column, "span start column")?,
        end_line: u32_value(span.end_line, "span end line")?,
        end_column: u32_value(span.end_column, "span end column")?,
    }))
}

fn u32_value(value: u64, field: &'static str) -> Result<u32, LexiconSnapshotError> {
    u32::try_from(value).map_err(|_| LexiconSnapshotError::Malformed(field))
}

fn normalize_path(path: &str) -> Result<String, LexiconSnapshotError> {
    normalize_repository_path(path).map_err(|_| LexiconSnapshotError::InvalidPath {
        field: "fact",
        path: path.to_owned(),
    })
}

fn validate_sha256_id(value: &str) -> Result<(), LexiconSnapshotError> {
    let Some(digest) = value.strip_prefix("sha256:") else {
        return Err(LexiconSnapshotError::InvalidId(value.to_owned()));
    };
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(LexiconSnapshotError::InvalidId(value.to_owned()));
    }
    Ok(())
}

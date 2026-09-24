use crate::{Analysis, EdgeRecord, FactHeader, FactRecord, NodeRecord, UnresolvedRecord};

use super::ScanExecutionError;
use super::legacy_wire::{LegacyHeader, LegacyRecord};

pub(crate) fn parse_legacy_analysis(
    input: &str,
    language: &str,
) -> Result<Analysis, ScanExecutionError> {
    let mut lines = input.lines().filter(|line| !line.trim().is_empty());
    let header_line = lines
        .next()
        .ok_or_else(|| ScanExecutionError::new("legacy adapter output is empty"))?;
    let wire: LegacyHeader = serde_json::from_str(header_line).map_err(|error| {
        ScanExecutionError::new(format!("decode legacy adapter header: {error}"))
    })?;
    validate_header(&wire, language)?;

    let mut records = Vec::new();
    for line in lines {
        let wire: LegacyRecord = serde_json::from_str(line).map_err(|error| {
            ScanExecutionError::new(format!("decode legacy adapter record: {error}"))
        })?;
        records.push(record(wire)?);
    }

    Ok(Analysis {
        header: FactHeader {
            adapter_version: wire.adapter_version,
            changed_files: wire.changed_files,
            language: wire.language,
            mode: non_empty(wire.mode),
            record: wire.record,
            removed_files: wire.removed_files,
            repository: wire.repository,
            schema_version: wire.schema_version,
            shared_complete: wire.shared_complete,
        },
        records,
    })
}

fn validate_header(header: &LegacyHeader, language: &str) -> Result<(), ScanExecutionError> {
    if header.record != "lexicon"
        || header.schema_version != 1
        || header.adapter_version.is_empty()
        || header.language.is_empty()
        || header.repository.is_empty()
    {
        return Err(ScanExecutionError::new(
            "invalid legacy Lexicon adapter header",
        ));
    }
    if header.language != language {
        return Err(ScanExecutionError::new(format!(
            "legacy adapter output language {:?} does not match {:?}",
            header.language, language
        )));
    }
    if !matches!(header.mode.as_str(), "" | "full" | "incremental") {
        return Err(ScanExecutionError::new(format!(
            "unsupported legacy adapter output mode {:?}",
            header.mode
        )));
    }
    if header.mode == "incremental"
        && (header.changed_files.is_none()
            || header.removed_files.is_none()
            || header.shared_complete.is_none())
    {
        return Err(ScanExecutionError::new(
            "incremental legacy adapter output has incomplete scope",
        ));
    }
    Ok(())
}

fn record(wire: LegacyRecord) -> Result<FactRecord, ScanExecutionError> {
    let span = wire.span.map(Into::into);
    match wire.record.as_str() {
        "node" => Ok(FactRecord::Node(NodeRecord {
            attributes: wire.attributes,
            content_id: non_empty(wire.content_id),
            id: wire.id,
            kind: wire.kind,
            name: wire.name,
            owner: non_empty(wire.owner),
            path: wire.path,
            qualified_name: wire.qualified_name,
            span,
        })),
        "edge" => Ok(FactRecord::Edge(EdgeRecord {
            attributes: wire.attributes,
            owner: non_empty(wire.owner),
            relation: wire.relation,
            source: wire.source,
            span,
            target: wire.target,
        })),
        "unresolved" => Ok(FactRecord::Unresolved(UnresolvedRecord {
            attributes: wire.attributes,
            candidate_name: non_empty(wire.candidate_name),
            candidate_namespace: non_empty(wire.candidate_namespace),
            expression: wire.expression,
            owner: non_empty(wire.owner),
            reason: wire.reason,
            relation: wire.relation,
            source: wire.source,
            span,
        })),
        other => Err(ScanExecutionError::new(format!(
            "unsupported legacy Lexicon record {other:?}"
        ))),
    }
}

fn non_empty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

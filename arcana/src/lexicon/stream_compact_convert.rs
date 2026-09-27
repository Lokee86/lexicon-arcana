use super::LexiconSnapshotError;
use super::binary_v2_reader::SpanRef;
use super::stream_compact::CompatibilityCounts;
use crate::repository::{NodeKind, RelationKind, normalize_repository_path};
use crate::repository_store::format::{
    UNKNOWN_REASON_CODE, node_kind_code, relation_code as store_relation_code,
};
use crate::repository_store::{CompactRepositoryAssembler, TempSpan};

pub(super) fn node_kind(value: &str, compatibility: &mut CompatibilityCounts) -> u16 {
    let kind = NodeKind::parse(value).unwrap_or_else(|| {
        *compatibility
            .entry(format!(
                "unrecognized Lexicon node kind {value:?}; treating as symbol"
            ))
            .or_default() += 1;
        NodeKind::Symbol
    });
    node_kind_code(&kind)
}

pub(super) fn relation_code(
    value: &str,
    record: &str,
    compatibility: &mut CompatibilityCounts,
) -> Option<u16> {
    let Some(relation) = RelationKind::parse(value) else {
        let message = if record == "edge" {
            format!("unrecognized Lexicon edge relation {value:?}; skipping edge")
        } else {
            format!("unrecognized Lexicon unresolved relation {value:?}; skipping record")
        };
        *compatibility.entry(message).or_default() += 1;
        return None;
    };
    Some(store_relation_code(&relation))
}

pub(super) fn unresolved_reason(
    value: &str,
    compatibility: &mut CompatibilityCounts,
) -> Result<(u16, bool), LexiconSnapshotError> {
    if value.is_empty() {
        return Err(LexiconSnapshotError::Malformed("empty unresolved reason"));
    }
    let code = match value {
        "missing-target" => 1,
        "ambiguous-target" => 2,
        "unsupported-form" => 3,
        "dynamic-target" => 4,
        "external-target" => 5,
        "builtin-target" => 6,
        "generated-target" => 7,
        "type-conversion" => 8,
        "self-target" => 9,
        "unsupported-macro-expansion" => 10,
        "macro-argument-mismatch" => 11,
        "macro-expansion-cycle" => 12,
        "macro-expansion-depth" => 13,
        "compiler-analysis-failed" => 14,
        "compiler-identity-mismatch" => 15,
        _ => {
            *compatibility
                .entry(format!(
                    "unrecognized Lexicon unresolved reason {value:?}; preserving label"
                ))
                .or_default() += 1;
            return Ok((UNKNOWN_REASON_CODE, true));
        }
    };
    Ok((code, false))
}

pub(super) fn compact_span(
    assembler: &mut CompactRepositoryAssembler,
    span: Option<SpanRef<'_>>,
) -> Result<Option<TempSpan>, LexiconSnapshotError> {
    span.map(|span| {
        let path = normalize_repository_path(span.path).map_err(|_| {
            LexiconSnapshotError::InvalidPath {
                field: "fact",
                path: span.path.to_owned(),
            }
        })?;
        Ok(TempSpan {
            path: assembler.intern(&path)?,
            start_line: u32_value(span.start_line, "span start line")?,
            start_column: u32_value(span.start_column, "span start column")?,
            end_line: u32_value(span.end_line, "span end line")?,
            end_column: u32_value(span.end_column, "span end column")?,
        })
    })
    .transpose()
}

fn u32_value(value: u64, field: &'static str) -> Result<u32, LexiconSnapshotError> {
    u32::try_from(value).map_err(|_| LexiconSnapshotError::Malformed(field))
}

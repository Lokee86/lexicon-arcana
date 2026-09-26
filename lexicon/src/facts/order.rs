use super::{FactRecord, SourceSpan};
use serde::Serialize;
use std::cmp::Ordering;

pub(crate) fn compare(left: &FactRecord, right: &FactRecord) -> Ordering {
    compare_primary(left, right).then_with(|| compare_tie_break(left, right))
}

pub(super) fn compare_primary(left: &FactRecord, right: &FactRecord) -> Ordering {
    rank(left)
        .cmp(&rank(right))
        .then_with(|| match (left, right) {
            (FactRecord::Node(left), FactRecord::Node(right)) => (
                &left.id,
                &left.kind,
                &left.path,
                &left.qualified_name,
            )
                .cmp(&(&right.id, &right.kind, &right.path, &right.qualified_name)),
            (FactRecord::Edge(left), FactRecord::Edge(right)) => (
                &left.source,
                &left.target,
                &left.relation,
                span_key(left.span.as_ref()),
            )
                .cmp(&(
                    &right.source,
                    &right.target,
                    &right.relation,
                    span_key(right.span.as_ref()),
                )),
            (FactRecord::Unresolved(left), FactRecord::Unresolved(right)) => (
                &left.source,
                &left.relation,
                &left.expression,
                &left.reason,
                span_key(left.span.as_ref()),
            )
                .cmp(&(
                    &right.source,
                    &right.relation,
                    &right.expression,
                    &right.reason,
                    span_key(right.span.as_ref()),
                )),
            _ => Ordering::Equal,
        })
}

fn compare_tie_break(left: &FactRecord, right: &FactRecord) -> Ordering {
    match (left, right) {
        (FactRecord::Node(left), FactRecord::Node(right)) => {
            compare_optional_json(left.attributes.as_ref(), right.attributes.as_ref())
                .then_with(|| {
                    compare_optional_json(left.content_id.as_ref(), right.content_id.as_ref())
                })
                .then_with(|| compare_json(&left.name, &right.name))
                .then_with(|| compare_optional_json(left.owner.as_ref(), right.owner.as_ref()))
                .then_with(|| compare_optional_json(left.span.as_ref(), right.span.as_ref()))
        }
        (FactRecord::Edge(left), FactRecord::Edge(right)) => {
            compare_optional_json(left.attributes.as_ref(), right.attributes.as_ref())
                .then_with(|| compare_optional_json(left.owner.as_ref(), right.owner.as_ref()))
        }
        (FactRecord::Unresolved(left), FactRecord::Unresolved(right)) => {
            compare_optional_json(left.attributes.as_ref(), right.attributes.as_ref())
                .then_with(|| {
                    compare_optional_json(
                        left.candidate_name.as_ref(),
                        right.candidate_name.as_ref(),
                    )
                })
                .then_with(|| {
                    compare_optional_json(
                        left.candidate_namespace.as_ref(),
                        right.candidate_namespace.as_ref(),
                    )
                })
                .then_with(|| compare_optional_json(left.owner.as_ref(), right.owner.as_ref()))
        }
        _ => Ordering::Equal,
    }
}

fn compare_optional_json<T: Serialize>(left: Option<&T>, right: Option<&T>) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => compare_json(left, right),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_json<T: Serialize>(left: &T, right: &T) -> Ordering {
    json_bytes(left).cmp(&json_bytes(right))
}

fn json_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).expect("fact fields must be JSON serializable")
}

fn rank(record: &FactRecord) -> u8 {
    match record {
        FactRecord::Node(_) => 0,
        FactRecord::Edge(_) => 1,
        FactRecord::Unresolved(_) => 2,
    }
}

fn span_key(span: Option<&SourceSpan>) -> (&str, u64, u64, u64, u64) {
    match span {
        Some(span) => (
            &span.path,
            span.start_line,
            span.start_column,
            span.end_line,
            span.end_column,
        ),
        None => ("", 0, 0, 0, 0),
    }
}

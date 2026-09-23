use super::{FactRecord, SourceSpan};
use std::cmp::Ordering;

pub(crate) fn compare(left: &FactRecord, right: &FactRecord) -> Ordering {
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

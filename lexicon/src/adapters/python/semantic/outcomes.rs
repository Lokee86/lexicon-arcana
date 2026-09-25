use std::collections::BTreeSet;

use super::super::facts::Facts;
use super::super::model::CallInfo;
use crate::SourceSpan;

pub fn emit(calls: &[CallInfo], facts: &mut Facts) {
    let async_ids = facts
        .functions
        .values()
        .filter(|info| info.is_async)
        .map(|info| info.node_id.clone())
        .collect::<BTreeSet<_>>();
    if async_ids.is_empty() {
        return;
    }

    let proven_spans = facts
        .edges
        .values()
        .filter(|edge| edge.relation == "calls" && async_ids.contains(&edge.target))
        .filter_map(|edge| edge.span.as_ref())
        .map(span_key)
        .collect::<BTreeSet<_>>();
    if proven_spans.is_empty() {
        return;
    }

    for call in calls {
        if !call.outcome_eligible {
            continue;
        }
        let Some(record_span) = call.span.clone() else {
            continue;
        };
        if !proven_spans.contains(&span_key(&record_span)) {
            continue;
        }

        let line = record_span.start_line;
        let column = record_span.start_column.saturating_sub(1);
        let identity = format!(
            "@semantic/outcome-operation/python/{}:{line}:{column}",
            record_span.path
        );
        let operation = facts.add_node(
            "protocol",
            "outcome-operation:python:async",
            &record_span.path,
            &identity,
            Some(&identity),
            Some(record_span.clone()),
            None,
            None,
        );
        if call.bare_expression {
            continue;
        }
        let action_identity = format!("{identity}/consume:{line}:{column}");
        let action = facts.add_node(
            "protocol",
            "outcome-action:consume",
            &record_span.path,
            &action_identity,
            Some(&action_identity),
            Some(record_span.clone()),
            None,
            None,
        );
        facts.add_edge(&operation, &action, "contains", Some(record_span), None);
    }
}

fn span_key(span: &SourceSpan) -> (String, u64, u64, u64, u64) {
    (
        span.path.clone(),
        span.start_line,
        span.start_column,
        span.end_line,
        span.end_column,
    )
}

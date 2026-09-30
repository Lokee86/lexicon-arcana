use std::collections::BTreeMap;

use crate::FactRecord;
use serde_json::{Map, Value, json};

pub(super) fn deduplicate(records: Vec<FactRecord>) -> Vec<FactRecord> {
    let mut unique = BTreeMap::<String, FactRecord>::new();
    for record in records {
        let key = match &record {
            FactRecord::Node(node) => format!("node\0{}", node.id),
            FactRecord::Edge(edge) => format!(
                "edge\0{}\0{}\0{}\0{}",
                edge.source,
                edge.target,
                edge.relation,
                span_key(edge.span.as_ref())
            ),
            FactRecord::Unresolved(value) => format!(
                "unresolved\0{}\0{}\0{}\0{}\0{}",
                value.source,
                value.relation,
                value.expression,
                value.reason,
                span_key(value.span.as_ref())
            ),
        };
        if let Some(existing) = unique.get_mut(&key) {
            merge_duplicate(existing, record);
        } else {
            unique.insert(key, record);
        }
    }
    unique.into_values().collect()
}

fn merge_duplicate(existing: &mut FactRecord, incoming: FactRecord) {
    match (existing, incoming) {
        (FactRecord::Edge(existing), FactRecord::Edge(incoming))
            if matches!(existing.relation.as_str(), "calls" | "possible-calls") =>
        {
            merge_call_attributes(&mut existing.attributes, incoming.attributes.as_ref());
        }
        (existing, incoming) => *existing = incoming,
    }
}

fn merge_call_attributes(existing: &mut Option<Value>, incoming: Option<&Value>) {
    let (Some(Value::Object(existing)), Some(Value::Object(incoming))) =
        (existing.as_mut(), incoming)
    else {
        return;
    };

    merge_bool(existing, incoming, "overload_selected");
    merge_number_max(existing, incoming, "argument_count");
    merge_number_max(existing, incoming, "candidate_count");
    merge_number_max(existing, incoming, "compiler_candidate_count");
    merge_string_set(existing, incoming, "evidence");
    merge_string_set(existing, incoming, "pointer_via");
    merge_preferred_type_name(existing, incoming, "receiver_type");
    merge_preferred_string(existing, incoming, "receiver_type_id");
}

fn merge_bool(existing: &mut Map<String, Value>, incoming: &Map<String, Value>, key: &str) {
    let value = existing.get(key).and_then(Value::as_bool).unwrap_or(false)
        || incoming.get(key).and_then(Value::as_bool).unwrap_or(false);
    existing.insert(key.into(), Value::Bool(value));
}

fn merge_number_max(existing: &mut Map<String, Value>, incoming: &Map<String, Value>, key: &str) {
    let left = existing.get(key).and_then(Value::as_u64).unwrap_or(0);
    let right = incoming.get(key).and_then(Value::as_u64).unwrap_or(0);
    existing.insert(key.into(), json!(left.max(right)));
}

fn merge_string_set(existing: &mut Map<String, Value>, incoming: &Map<String, Value>, key: &str) {
    let mut values = existing
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .chain(
            incoming
                .get(key)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str),
        )
        .map(str::to_owned)
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    if !values.is_empty() {
        existing.insert(key.into(), json!(values));
    }
}

fn merge_preferred_type_name(
    existing: &mut Map<String, Value>,
    incoming: &Map<String, Value>,
    key: &str,
) {
    let left = existing.get(key).and_then(Value::as_str).unwrap_or("");
    let right = incoming.get(key).and_then(Value::as_str).unwrap_or("");
    let left_rank = (left.matches("::").count(), left.len());
    let right_rank = (right.matches("::").count(), right.len());
    let preferred = match left_rank.cmp(&right_rank) {
        std::cmp::Ordering::Greater => left,
        std::cmp::Ordering::Less => right,
        std::cmp::Ordering::Equal => left.min(right),
    };
    existing.insert(key.into(), Value::String(preferred.into()));
}

fn merge_preferred_string(
    existing: &mut Map<String, Value>,
    incoming: &Map<String, Value>,
    key: &str,
) {
    let left = existing.get(key).and_then(Value::as_str).unwrap_or("");
    let right = incoming.get(key).and_then(Value::as_str).unwrap_or("");
    let preferred = match (left.is_empty(), right.is_empty()) {
        (true, false) => right,
        (false, true) => left,
        _ => left.min(right),
    };
    existing.insert(key.into(), Value::String(preferred.into()));
}

fn span_key(span: Option<&crate::SourceSpan>) -> String {
    span.map(|span| {
        format!(
            "{}\0{:08}\0{:08}\0{:08}\0{:08}",
            span.path, span.start_line, span.start_column, span.end_line, span.end_column
        )
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use crate::{EdgeRecord, FactRecord, SourceSpan};
    use serde_json::{Value, json};

    use super::deduplicate;

    fn call_edge(receiver_type: &str, overload_selected: bool) -> FactRecord {
        FactRecord::Edge(EdgeRecord {
            attributes: Some(json!({
                "argument_count": 2,
                "candidate_count": 1,
                "compiler_candidate_count": 1,
                "dispatch": "static",
                "evidence": ["clang", "clang-operator"],
                "overload_selected": overload_selected,
                "receiver_type": receiver_type,
                "receiver_type_id": "type-id",
                "resolution": "definite"
            })),
            owner: Some("header.hpp".into()),
            relation: "calls".into(),
            source: "source".into(),
            span: Some(SourceSpan {
                path: "header.hpp".into(),
                start_line: 10,
                start_column: 5,
                end_line: 10,
                end_column: 20,
            }),
            target: "target".into(),
        })
    }

    #[test]
    fn duplicate_call_edges_merge_contextual_metadata_deterministically() {
        let qualified = call_edge("Catch::ReusableStringStream", true);
        let unqualified = call_edge("ReusableStringStream", false);

        let forward = deduplicate(vec![qualified.clone(), unqualified.clone()]);
        let reverse = deduplicate(vec![unqualified, qualified]);

        assert_eq!(forward, reverse);
        let FactRecord::Edge(edge) = &forward[0] else {
            panic!("expected call edge");
        };
        let attributes = edge
            .attributes
            .as_ref()
            .and_then(Value::as_object)
            .expect("call attributes");
        assert_eq!(
            attributes.get("receiver_type").and_then(Value::as_str),
            Some("Catch::ReusableStringStream")
        );
        assert_eq!(
            attributes.get("overload_selected").and_then(Value::as_bool),
            Some(true)
        );
    }
}

use super::model::{EdgeRecord, FactRecord, NodeRecord, SourceSpan, UnresolvedRecord};
use super::order::{compare, compare_primary};
use serde::Serialize;
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[test]
fn structural_total_order_matches_legacy_json_tie_break() {
    let span_a = SourceSpan {
        path: "a.go".into(),
        start_line: 1,
        start_column: 2,
        end_line: 1,
        end_column: 4,
    };
    let span_b = SourceSpan {
        path: "a.go".into(),
        start_line: 1,
        start_column: 2,
        end_line: 2,
        end_column: 1,
    };
    let records = vec![
        node(None, None, "plain", None, None),
        node(Some(json!({"z": 1, "a": 2})), None, "plain", None, None),
        node(None, Some("content"), "plain", None, None),
        node(None, None, "quote\"name", None, None),
        node(None, None, "slash\\name", None, None),
        node(None, None, "plain", Some("a.go"), None),
        node(None, None, "plain", None, Some(span_a.clone())),
        node(None, None, "plain", None, Some(span_b.clone())),
        edge(None, None),
        edge(Some(json!({"weight": 2})), None),
        edge(None, Some("a.go")),
        unresolved(None, None, None, None),
        unresolved(Some(json!({"reason": "x"})), None, None, None),
        unresolved(None, Some("candidate\"name"), None, None),
        unresolved(None, None, Some("example.com/pkg"), None),
        unresolved(None, None, None, Some("a.go")),
    ];

    for left in &records {
        for right in &records {
            assert_eq!(
                compare(left, right),
                legacy_compare(left, right),
                "new structural comparator diverged for {left:?} versus {right:?}"
            );
        }
    }
}

#[test]
fn total_order_distinguishes_records_that_share_primary_keys() {
    let left = node(None, None, "a", None, None);
    let right = node(None, None, "b", None, None);

    assert_eq!(compare_primary(&left, &right), Ordering::Equal);
    assert_ne!(compare(&left, &right), Ordering::Equal);
}

fn node(
    attributes: Option<Value>,
    content_id: Option<&str>,
    name: &str,
    owner: Option<&str>,
    span: Option<SourceSpan>,
) -> FactRecord {
    FactRecord::Node(NodeRecord {
        attributes,
        content_id: content_id.map(str::to_owned),
        id: "1111111111111111111111111111111111111111111111111111111111111111".into(),
        kind: "function".into(),
        name: name.into(),
        owner: owner.map(str::to_owned),
        path: "a.go".into(),
        qualified_name: "a.go::f".into(),
        span,
    })
}

fn edge(attributes: Option<Value>, owner: Option<&str>) -> FactRecord {
    FactRecord::Edge(EdgeRecord {
        attributes,
        owner: owner.map(str::to_owned),
        relation: "calls".into(),
        source: "1111111111111111111111111111111111111111111111111111111111111111".into(),
        span: None,
        target: "2222222222222222222222222222222222222222222222222222222222222222".into(),
    })
}

fn unresolved(
    attributes: Option<Value>,
    candidate_name: Option<&str>,
    candidate_namespace: Option<&str>,
    owner: Option<&str>,
) -> FactRecord {
    FactRecord::Unresolved(UnresolvedRecord {
        attributes,
        candidate_name: candidate_name.map(str::to_owned),
        candidate_namespace: candidate_namespace.map(str::to_owned),
        expression: "f()".into(),
        owner: owner.map(str::to_owned),
        reason: "missing-target".into(),
        relation: "calls".into(),
        source: "1111111111111111111111111111111111111111111111111111111111111111".into(),
        span: None,
    })
}

fn legacy_compare(left: &FactRecord, right: &FactRecord) -> Ordering {
    compare_primary(left, right).then_with(|| {
        legacy_tagged_bytes(left)
            .expect("test record must serialize")
            .cmp(&legacy_tagged_bytes(right).expect("test record must serialize"))
    })
}

fn legacy_tagged_bytes(record: &FactRecord) -> Result<Vec<u8>, serde_json::Error> {
    let value = match record {
        FactRecord::Node(value) => legacy_tagged_value("node", value)?,
        FactRecord::Edge(value) => legacy_tagged_value("edge", value)?,
        FactRecord::Unresolved(value) => legacy_tagged_value("unresolved", value)?,
    };
    serde_json::to_vec(&value)
}

fn legacy_tagged_value<T: Serialize>(record: &str, value: &T) -> Result<Value, serde_json::Error> {
    let Value::Object(map) = serde_json::to_value(value)? else {
        unreachable!("fact records serialize as JSON objects");
    };
    let mut sorted = BTreeMap::new();
    for (key, value) in map {
        sorted.insert(key, value);
    }
    sorted.insert("record".into(), Value::String(record.into()));
    serde_json::to_value(sorted)
}

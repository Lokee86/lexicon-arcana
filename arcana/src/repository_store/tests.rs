use crate::repository::{
    ContentId, NodeFact, NodeKey, NodeKind, RelationKind, RepositoryFacts, SourceSpan,
    UnresolvedReason, UnresolvedReferenceFact,
};

use super::*;

#[test]
fn string_table_is_deterministic_and_preserves_weird_strings() {
    let values = [
        "plain",
        "-",
        "tab\tnewline\nreturn\rslash\\nul\0snowman☃",
        "",
        "qualified::name",
    ];
    let mut forward = StringTableBuilder::default();
    for value in values {
        forward.insert(value);
    }
    let mut reverse = StringTableBuilder::default();
    for value in values.into_iter().rev() {
        reverse.insert(value);
    }

    let forward = forward.finish().unwrap();
    let reverse = reverse.finish().unwrap();
    assert_eq!(forward, reverse);
    assert_eq!(forward.encode().unwrap(), reverse.encode().unwrap());

    let encoded = forward.encode().unwrap();
    let decoded = CompactStringTable::decode(&encoded, forward.len() as u64).unwrap();
    assert_eq!(decoded, forward);
    for value in values {
        assert_eq!(decoded.get(decoded.id(value).unwrap()).unwrap(), value);
    }
    assert_ne!(decoded.id("-").unwrap(), StringId::ABSENT);
}

#[test]
fn sha256_identity_round_trips_raw_digest() {
    let text = format!("sha256:{}", "0a".repeat(32));
    let identity = Sha256Identity::parse(&text).unwrap();
    assert_eq!(identity.0, [0x0a; 32]);
    assert_eq!(identity.canonical_string(), text);
    assert_eq!(
        Sha256Identity::parse(&format!("sha256:{}", "AA".repeat(32))),
        Err(StoreFormatError::InvalidExternalIdentity)
    );
}

#[test]
fn compact_node_round_trips_all_metadata() {
    let node = NodeFact {
        key: NodeKey::from_u64(0x1122),
        external_identity: Some(format!("sha256:{}", "ab".repeat(32))),
        kind: NodeKind::Function,
        path: "src/weird.rs".into(),
        name: "odd\tname\n\\\0☃".into(),
        qualified_name: "crate::odd\tname\n\\\0☃".into(),
        content_id: Some(ContentId::from_u64(0)),
        span: Some(SourceSpan {
            path: "src/weird.rs".into(),
            start_line: 7,
            start_column: 0,
            end_line: 9,
            end_column: u32::MAX,
        }),
    };
    let facts = RepositoryFacts::new(vec![node.clone()], Vec::new());
    let strings = table_for(&facts);

    let compact = CompactNodeRecord::from_fact(&node, &strings, 3).unwrap();
    let encoded = compact.encode();
    assert_eq!(encoded.len(), format::NODE_RECORD_LEN as usize);
    let decoded = CompactNodeRecord::decode(&encoded).unwrap();
    assert_eq!(decoded.occurrence_count, 3);
    assert_eq!(decoded.to_fact(&strings).unwrap(), node);
}

#[test]
fn compact_node_preserves_absent_optional_values() {
    let node = NodeFact {
        key: NodeKey::from_u64(7),
        external_identity: None,
        kind: NodeKind::File,
        path: "src/lib.rs".into(),
        name: "lib.rs".into(),
        qualified_name: "".into(),
        content_id: None,
        span: None,
    };
    let facts = RepositoryFacts::new(vec![node.clone()], Vec::new());
    let strings = table_for(&facts);
    let encoded = CompactNodeRecord::from_fact(&node, &strings, 1)
        .unwrap()
        .encode();
    let decoded = CompactNodeRecord::decode(&encoded).unwrap();
    assert_eq!(decoded.external_identity, None);
    assert_eq!(decoded.content_id, None);
    assert_eq!(decoded.span, None);
    assert_eq!(decoded.to_fact(&strings).unwrap(), node);
}

#[test]
fn unresolved_round_trips_unknown_reason_and_sentinel_like_strings() {
    let reference = UnresolvedReferenceFact {
        source: NodeKey::from_u64(9),
        relation: RelationKind::PossibleCalls,
        expression: "callee\twith\ncontrols\\\0☃".into(),
        candidate_namespace: Some("-".into()),
        candidate_name: None,
        reason: UnresolvedReason::Unknown("future\treason".into()),
        span: Some(SourceSpan {
            path: "src/call.rs".into(),
            start_line: 10,
            start_column: 3,
            end_line: 10,
            end_column: 17,
        }),
    };
    let facts = RepositoryFacts::with_unresolved(Vec::new(), Vec::new(), vec![reference.clone()]);
    let strings = table_for(&facts);

    let encoded = CompactUnresolvedRecord::from_fact(&reference, &strings)
        .unwrap()
        .encode();
    assert_eq!(encoded.len(), format::UNRESOLVED_RECORD_LEN as usize);
    let decoded = CompactUnresolvedRecord::decode(&encoded).unwrap();
    assert_ne!(decoded.candidate_namespace, StringId::ABSENT);
    assert_eq!(decoded.candidate_name, StringId::ABSENT);
    assert_eq!(decoded.to_fact(&strings).unwrap(), reference);
}

#[test]
fn unresolved_round_trips_absent_fields_and_known_reason() {
    let reference = UnresolvedReferenceFact {
        source: NodeKey::from_u64(11),
        relation: RelationKind::References,
        expression: "missing".into(),
        candidate_namespace: None,
        candidate_name: None,
        reason: UnresolvedReason::MissingTarget,
        span: None,
    };
    let facts = RepositoryFacts::with_unresolved(Vec::new(), Vec::new(), vec![reference.clone()]);
    let strings = table_for(&facts);
    let encoded = CompactUnresolvedRecord::from_fact(&reference, &strings)
        .unwrap()
        .encode();
    let decoded = CompactUnresolvedRecord::decode(&encoded).unwrap();
    assert_eq!(decoded.unknown_reason, StringId::ABSENT);
    assert_eq!(decoded.to_fact(&strings).unwrap(), reference);
}

#[test]
fn fact_collection_interns_repeated_strings_once() {
    let node = NodeFact {
        key: NodeKey::from_u64(1),
        external_identity: None,
        kind: NodeKind::Function,
        path: "same".into(),
        name: "same".into(),
        qualified_name: "same".into(),
        content_id: None,
        span: Some(SourceSpan {
            path: "same".into(),
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 2,
        }),
    };
    let facts = RepositoryFacts::new(vec![node], Vec::new());
    let strings = table_for(&facts);
    assert_eq!(strings.len(), 1);
    assert_eq!(strings.get(StringId(0)).unwrap(), "same");
}

fn table_for(facts: &RepositoryFacts) -> CompactStringTable {
    let mut builder = StringTableBuilder::default();
    builder.collect_facts(facts);
    builder.finish().unwrap()
}

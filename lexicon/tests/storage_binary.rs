use lexicon::{
    EdgeRecord, FactObject, FactRecord, NodeRecord, SourceSpan, UnresolvedRecord,
    decode_node_facts, decode_object, encode_object, object_id,
};

const GO_GOLDENS: &str = include_str!("../internal/objectstore/binary_golden_test.go");

#[test]
fn v2_writer_matches_go_golden_bytes() {
    let expected = golden_hex("binaryObjectV2GoldenHex");
    let encoded = encode_object(&golden_object()).expect("encode v2 object");
    assert_eq!(encoded, expected);
}

#[test]
fn v2_reader_matches_go_golden_object() {
    let decoded = decode_object(&golden_hex("binaryObjectV2GoldenHex")).expect("decode v2 object");
    assert_eq!(decoded, golden_object());
}

#[test]
fn v1_reader_matches_go_legacy_golden_object() {
    let decoded = decode_object(&golden_hex("binaryObjectGoldenHex")).expect("decode v1 object");
    assert_eq!(decoded, golden_object());
}

#[test]
fn node_only_reader_skips_relationship_materialization() {
    let encoded = golden_hex("binaryObjectV2GoldenHex");
    let (metadata, nodes) = decode_node_facts(&encoded).expect("decode node-only object");
    assert!(metadata.records.is_empty());
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0], file_node());
    assert_eq!(nodes[1], function_node());
}

#[test]
fn binary_reader_rejects_truncation_and_trailing_bytes() {
    let encoded = golden_hex("binaryObjectV2GoldenHex");
    assert!(decode_object(&encoded[..encoded.len() - 1]).is_err());
    let mut trailing = encoded;
    trailing.push(0);
    assert!(decode_object(&trailing).is_err());
}

#[test]
fn object_identity_uses_existing_domain() {
    let encoded = golden_hex("binaryObjectV2GoldenHex");
    assert_eq!(
        object_id(&encoded),
        "sha256:ce7cc690ad0bc0e15c5b28ae17fbc39d4bcd19a933cab9dd79e6fbaaebe6821c"
    );
}

fn golden_object() -> FactObject {
    FactObject {
        version: 1,
        language: "go".into(),
        owner: "main.go".into(),
        source_content_id: format!("sha256:{}", "b".repeat(64)),
        adapter_version: "1.0.0".into(),
        schema_version: 1,
        analysis_config_id: format!("sha256:{}", "c".repeat(64)),
        records: vec![
            FactRecord::Node(file_node()),
            FactRecord::Node(function_node()),
            FactRecord::Edge(EdgeRecord {
                attributes: None,
                owner: Some("main.go".into()),
                relation: "contains".into(),
                source: format!("sha256:{}", "2".repeat(64)),
                span: None,
                target: format!("sha256:{}", "1".repeat(64)),
            }),
            FactRecord::Unresolved(UnresolvedRecord {
                attributes: None,
                candidate_name: Some("f".into()),
                candidate_namespace: None,
                expression: "f()".into(),
                owner: Some("main.go".into()),
                reason: "dynamic-target".into(),
                relation: "calls".into(),
                source: format!("sha256:{}", "2".repeat(64)),
                span: None,
            }),
        ],
    }
}

fn file_node() -> NodeRecord {
    NodeRecord {
        attributes: None,
        content_id: Some(format!("sha256:{}", "b".repeat(64))),
        id: format!("sha256:{}", "1".repeat(64)),
        kind: "file".into(),
        name: "main.go".into(),
        owner: Some("main.go".into()),
        path: "main.go".into(),
        qualified_name: "main.go".into(),
        span: None,
    }
}

fn function_node() -> NodeRecord {
    NodeRecord {
        attributes: None,
        content_id: None,
        id: format!("sha256:{}", "2".repeat(64)),
        kind: "function".into(),
        name: "main".into(),
        owner: Some("main.go".into()),
        path: "main.go".into(),
        qualified_name: "demo.main".into(),
        span: Some(SourceSpan {
            end_column: 2,
            end_line: 2,
            path: "main.go".into(),
            start_column: 1,
            start_line: 2,
        }),
    }
}

fn golden_hex(name: &str) -> Vec<u8> {
    let prefix = format!("const {name} = \"");
    let start = GO_GOLDENS
        .find(&prefix)
        .map(|index| index + prefix.len())
        .expect("Go golden constant");
    let end = GO_GOLDENS[start..]
        .find('"')
        .map(|index| start + index)
        .expect("Go golden closing quote");
    decode_hex(&GO_GOLDENS[start..end])
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).unwrap())
        .collect()
}

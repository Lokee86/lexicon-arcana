use lexicon::{
    EdgeRecord, FactObject, FactRecord, NodeRecord, UnresolvedRecord, decode_object, encode_object,
};
use serde_json::json;

#[test]
fn v2_round_trip_preserves_fallback_strings_and_external_references() {
    let external_sha = format!("sha256:{}", "d".repeat(64));
    let object = FactObject {
        version: 99,
        language: "custom".into(),
        owner: "src/main.ext".into(),
        source_content_id: "content-key".into(),
        adapter_version: "adapter-dev".into(),
        schema_version: 7,
        analysis_config_id: "config-key".into(),
        records: vec![
            FactRecord::Node(NodeRecord {
                attributes: Some(json!({"role": "entry"})),
                content_id: None,
                id: "node-local".into(),
                kind: "custom-kind".into(),
                name: "run".into(),
                owner: Some("src/main.ext".into()),
                path: "src/main.ext".into(),
                qualified_name: "demo.run".into(),
                span: None,
            }),
            FactRecord::Edge(EdgeRecord {
                attributes: Some(json!({"weight": 2})),
                owner: Some("src/main.ext".into()),
                relation: "links-to".into(),
                source: "node-local".into(),
                span: None,
                target: external_sha,
            }),
            FactRecord::Unresolved(UnresolvedRecord {
                attributes: None,
                candidate_name: Some("late".into()),
                candidate_namespace: Some("demo".into()),
                expression: "late()".into(),
                owner: Some("other.ext".into()),
                reason: "runtime-only".into(),
                relation: "dynamic-links".into(),
                source: "external-symbol".into(),
                span: None,
            }),
        ],
    };

    let encoded = encode_object(&object).expect("encode fallback object");
    assert_eq!(encoded, encode_object(&object).unwrap());

    let mut expected = object;
    expected.version = 1;
    assert_eq!(
        decode_object(&encoded).expect("decode fallback object"),
        expected
    );
}

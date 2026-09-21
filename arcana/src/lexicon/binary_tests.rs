use std::hint::black_box;
use std::time::Instant;

use super::binary::parse_binary_object;
use super::object::parse_json_object;
use super::records::build_repository_facts;

const GOLDEN_V1_HEX: &str = "4c584f424a0001000101110002676f076d61696e2e676f477368613235363a6262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626262626205312e302e30477368613235363a63636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363636363477368613235363a313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310466696c65477368613235363a323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232320866756e6374696f6e046d61696e0964656d6f2e6d61696e08636f6e7461696e730166036628290e64796e616d69632d7461726765740563616c6c7301020304051802000306070202020200000008090a02020b010202010202070100020c0800060a01000d000e020f100800";
const GOLDEN_V2_HEX: &str = "4c584f424a00020001010900000005312e302e30000964656d6f2e6d61696e010d796e616d69632d746172676574000166010228290002676f00046d61696e04032e676f060801bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb0101cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc007a020001bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb0111111111111111111111111111111111111111111111111111111111111111110308010101000000000122222222222222222222222222222222222222222222222222222222222222220b0701010002010802010202090100010100020000010b0100040005010304000200";

const GOLDEN_JSON: &str = r#"{
  "version": 1,
  "language": "go",
  "owner": "main.go",
  "source_content_id": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
  "adapter_version": "1.0.0",
  "schema_version": 1,
  "analysis_config_id": "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
  "records": [
    {"content_id":"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","id":"sha256:1111111111111111111111111111111111111111111111111111111111111111","kind":"file","name":"main.go","owner":"main.go","path":"main.go","qualified_name":"main.go","record":"node"},
    {"id":"sha256:2222222222222222222222222222222222222222222222222222222222222222","kind":"function","name":"main","owner":"main.go","path":"main.go","qualified_name":"demo.main","record":"node","span":{"end_column":2,"end_line":2,"path":"main.go","start_column":1,"start_line":2}},
    {"owner":"main.go","record":"edge","relation":"contains","source":"sha256:2222222222222222222222222222222222222222222222222222222222222222","target":"sha256:1111111111111111111111111111111111111111111111111111111111111111"},
    {"candidate_name":"f","expression":"f()","owner":"main.go","reason":"dynamic-target","record":"unresolved","relation":"calls","source":"sha256:2222222222222222222222222222222222222222222222222222222222222222"}
  ]
}"#;

#[test]
fn reads_go_produced_binary_v1_golden_object() {
    assert_golden_object(GOLDEN_V1_HEX);
}

#[test]
fn reads_go_produced_binary_v2_golden_object() {
    assert_golden_object(GOLDEN_V2_HEX);
}

fn assert_golden_object(hex: &str) {
    let bytes = decode_hex(hex);
    let object = parse_binary_object(&bytes).unwrap();
    assert_eq!(object.version, 1);
    assert_eq!(object.language, "go");
    assert_eq!(object.owner.as_deref(), Some("main.go"));
    assert_eq!(
        object.source_content_id.as_deref(),
        Some("sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
    );
    assert_eq!(object.adapter_version, "1.0.0");
    assert_eq!(object.schema_version, 1);
    assert_eq!(
        object.analysis_config_id,
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
    );

    let (facts, warnings) = build_repository_facts(object.records).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(facts.nodes.len(), 2);
    assert_eq!(facts.edges.len(), 1);
    assert_eq!(facts.unresolved.len(), 1);
}

#[test]
fn rejects_trailing_binary_object_bytes() {
    let mut bytes = decode_hex(GOLDEN_V2_HEX);
    bytes.push(0);
    assert!(parse_binary_object(&bytes).is_err());
}

#[test]
#[ignore = "manual release-mode performance benchmark"]
fn benchmark_binary_and_json_ingestion() {
    const ROUNDS: u32 = 50_000;
    let binary = decode_hex(GOLDEN_V2_HEX);

    let started = Instant::now();
    for _ in 0..ROUNDS {
        let object = parse_binary_object(black_box(&binary)).unwrap();
        black_box(build_repository_facts(object.records).unwrap());
    }
    let binary_elapsed = started.elapsed();

    let started = Instant::now();
    for _ in 0..ROUNDS {
        let object = parse_json_object(black_box(GOLDEN_JSON.as_bytes())).unwrap();
        black_box(build_repository_facts(object.records).unwrap());
    }
    let json_elapsed = started.elapsed();

    eprintln!(
        "Arcana Lexicon ingestion: binary={binary_elapsed:?}, JSON={json_elapsed:?}, rounds={ROUNDS}"
    );
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| (hex_digit(pair[0]) << 4) | hex_digit(pair[1]))
        .collect()
}

fn hex_digit(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("invalid test hex"),
    }
}

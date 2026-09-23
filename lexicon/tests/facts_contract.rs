use lexicon::{FactStream, ValidationError, node_id};

const REFERENCE_FACTS: &str = include_str!("../evaluation/rust_migration/fixtures/facts-v1.jsonl");

#[test]
fn parses_and_reemits_reference_facts_byte_identically() {
    let stream = FactStream::parse(REFERENCE_FACTS).expect("reference facts must validate");
    assert_eq!(stream.records.len(), 4);
    assert_eq!(
        stream.canonical_jsonl().expect("canonical output"),
        REFERENCE_FACTS.as_bytes()
    );
}

#[test]
fn rejects_noncanonical_record_order() {
    let mut stream = FactStream::parse(REFERENCE_FACTS).unwrap();
    stream.records.swap(0, 1);
    assert_eq!(stream.validate(), Err(ValidationError::NonCanonicalOrder));
}

#[test]
fn rejects_unknown_relationship_source() {
    let unknown = format!("sha256:{}", "f".repeat(64));
    let input = REFERENCE_FACTS.replace(
        "sha256:e774b7aef4d62e1f9c5f1b1a045d104fd913fb25b96924be36e27b4b51d63e1f\",\"target",
        &format!("{unknown}\",\"target"),
    );
    assert!(matches!(
        FactStream::parse(&input),
        Err(ValidationError::UnknownSource(id)) if id == unknown
    ));
}

#[test]
fn rejects_noncanonical_repository_paths() {
    let input = REFERENCE_FACTS.replace("\"owner\":\"main.py\"", "\"owner\":\"src\\\\main.py\"");
    assert!(matches!(
        FactStream::parse(&input),
        Err(ValidationError::InvalidPath(_))
    ));
}

#[test]
fn incremental_stream_requires_changed_owner_scope() {
    let function = node_id("python", "function", "demo.run");
    let input = format!(
        "{{\"adapter_version\":\"reference\",\"changed_files\":[\"other.py\"],\"language\":\"python\",\"mode\":\"incremental\",\"record\":\"lexicon\",\"removed_files\":[],\"repository\":\"fixture\",\"schema_version\":1,\"shared_complete\":false}}\n{{\"id\":\"{function}\",\"kind\":\"function\",\"name\":\"run\",\"owner\":\"main.py\",\"path\":\"main.py\",\"qualified_name\":\"demo.run\",\"record\":\"node\"}}\n"
    );
    assert!(matches!(
        FactStream::parse(&input),
        Err(ValidationError::InvalidIncrementalOwnership(path)) if path == "main.py"
    ));
}

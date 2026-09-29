use std::path::Path;

use lexicon::{AdapterHost, AdapterRequest, FactStream};

#[test]
fn frozen_phase2_c_family_oracle_matches_canonical_facts() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository = manifest.join("testdata").join("c_family_phase2_oracle");
    let host = AdapterHost::new(manifest.join("adapters"));

    let analysis = host
        .analyze(&AdapterRequest {
            language: "c-family".into(),
            repository,
            workers: 2,
            shards: 4,
            merge_fan_in: 2,
            ..Default::default()
        })
        .expect("analyze frozen C-family oracle");

    let actual = FactStream {
        header: analysis.header,
        records: analysis.records,
    }
    .canonical_jsonl()
    .expect("serialize frozen C-family oracle");
    let expected = include_bytes!("../testdata/c_family_phase2_oracle/facts.jsonl");

    assert_eq!(actual.as_slice(), expected);
}

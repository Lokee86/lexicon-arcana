use std::{fs, path::PathBuf};

use crate::{AdapterRequest, Analysis, FactHeader, LanguageAdapter};

use super::{
    GoAdapter,
    oracle_compare::{compare_records, format_differences},
    tests::real_helper,
};

const FIXTURES: &[&str] = &[
    "basic_calls",
    "relationships",
    "higher_order",
    "dataflow",
    "build_tags",
    "multi_module",
    "parallel",
];

#[test]
fn canonical_frontend_seam_matches_frozen_oracle_without_exceptions() {
    for name in FIXTURES {
        let repository = fixture(name);
        let expected = frozen_oracle(name);
        let actual = GoAdapter::with_frontend(real_helper())
            .analyze(&AdapterRequest {
                language: "go".into(),
                repository,
                workers: 1,
                shards: 1,
                merge_fan_in: 2,
                ..AdapterRequest::default()
            })
            .unwrap();

        expected.validate().unwrap();
        actual.validate().unwrap();
        assert_eq!(
            semantic_header(&actual.header),
            semantic_header(&expected.header),
            "semantic header mismatch for {name}"
        );

        let differences = compare_records(&expected.records, &actual.records);
        assert!(
            differences.is_empty(),
            "Go frontend seam regression for {name}; Phase 1 has no approved oracle              exceptions, intentional contract changes, or obsolete expectations:
{}",
            format_differences(&differences)
        );
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/go_oracle/repositories")
        .join(name)
}

fn frozen_oracle(name: &str) -> Analysis {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/go_oracle/golden")
        .join(format!("{name}.jsonl"));
    Analysis::parse(
        &fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read frozen Go oracle {}: {error}", path.display())),
    )
    .unwrap()
}

type SemanticHeader<'a> = (
    &'a str,
    Option<&'a str>,
    &'a str,
    u32,
    &'a [String],
    &'a [String],
    Option<bool>,
);

fn semantic_header(header: &FactHeader) -> SemanticHeader<'_> {
    (
        &header.language,
        header.mode.as_deref(),
        &header.repository,
        header.schema_version,
        header.changed_files.as_deref().unwrap_or_default(),
        header.removed_files.as_deref().unwrap_or_default(),
        header.shared_complete,
    )
}

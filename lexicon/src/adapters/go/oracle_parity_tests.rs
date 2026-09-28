use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{AdapterRequest, Analysis, FactHeader, LanguageAdapter};

use super::{
    GoAdapter,
    oracle_compare::{compare_records, format_differences},
    tests::real_helper,
};

#[derive(Debug, Clone, Copy)]
struct Execution {
    workers: usize,
    shards: usize,
    merge_fan_in: usize,
}

const SERIAL: Execution = Execution {
    workers: 1,
    shards: 1,
    merge_fan_in: 2,
};

#[test]
fn whole_fixture_suite_matches_frozen_oracle() {
    for name in [
        "basic_calls",
        "relationships",
        "higher_order",
        "dataflow",
        "build_tags",
        "multi_module",
        "parallel",
    ] {
        assert_case(name, SERIAL);
    }
}

#[test]
fn parallel_fixture_matches_frozen_oracle_across_execution_shapes() {
    for execution in [
        SERIAL,
        Execution {
            workers: 4,
            shards: 8,
            merge_fan_in: 4,
        },
        Execution {
            workers: 3,
            shards: 6,
            merge_fan_in: 8,
        },
    ] {
        assert_case("parallel", execution);
    }
}

fn assert_case(name: &str, execution: Execution) {
    let repository = fixture(name);
    let expected = frozen_oracle(name);
    let native = native_analysis(&repository, execution);
    expected.validate().unwrap();
    native.validate().unwrap();

    assert_eq!(
        semantic_header(&native.header),
        semantic_header(&expected.header),
        "semantic header mismatch for {name} at {}",
        execution_label(execution)
    );

    let differences = compare_records(&expected.records, &native.records);
    assert!(
        differences.is_empty(),
        "Go frozen-oracle parity failed for {name} at {}:\n{}",
        execution_label(execution),
        format_differences(&differences)
    );
}

fn native_analysis(repository: &Path, execution: Execution) -> Analysis {
    GoAdapter::with_frontend(real_helper())
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository: repository.to_path_buf(),
            workers: execution.workers,
            shards: execution.shards,
            merge_fan_in: execution.merge_fan_in,
            ..AdapterRequest::default()
        })
        .unwrap()
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

fn execution_label(execution: Execution) -> String {
    format!(
        "workers={} shards={} fan-in={}",
        execution.workers, execution.shards, execution.merge_fan_in
    )
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/go_oracle/repositories")
        .join(name)
}

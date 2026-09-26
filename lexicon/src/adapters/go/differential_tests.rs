use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

use crate::{AdapterRequest, Analysis, FactHeader, LanguageAdapter};

use super::{
    GoAdapter,
    differential_compare::{compare_records, format_differences},
    tests::{TempDirectory, real_helper},
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
fn whole_fixture_suite_matches_live_legacy_adapter() {
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
fn parallel_fixture_matches_live_legacy_across_execution_shapes() {
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
    let legacy = legacy_analysis(&repository, execution);
    let native = native_analysis(&repository, execution);
    legacy.validate().unwrap();
    native.validate().unwrap();

    assert_eq!(
        semantic_header(&native.header),
        semantic_header(&legacy.header),
        "semantic header mismatch for {name} at {}",
        execution_label(execution)
    );

    let differences = compare_records(&legacy.records, &native.records);
    assert!(
        differences.is_empty(),
        "Go differential parity failed for {name} at {}:\n{}",
        execution_label(execution),
        format_differences(&differences)
    );
}

fn native_analysis(repository: &Path, execution: Execution) -> Analysis {
    GoAdapter::with_helper(real_helper())
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

fn legacy_analysis(repository: &Path, execution: Execution) -> Analysis {
    let temporary = TempDirectory::new("differential-legacy");
    let output_path = temporary.path.join("facts.jsonl");
    let output = Command::new(legacy_adapter())
        .arg("-repo")
        .arg(repository)
        .arg("-output")
        .arg(&output_path)
        .arg("-workers")
        .arg(execution.workers.to_string())
        .arg("-shards")
        .arg(execution.shards.to_string())
        .arg("-merge-fan-in")
        .arg(execution.merge_fan_in.to_string())
        .output()
        .expect("run legacy Go adapter");
    assert!(
        output.status.success(),
        "legacy Go adapter failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Analysis::parse(&fs::read_to_string(output_path).unwrap()).unwrap()
}

fn legacy_adapter() -> &'static PathBuf {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        let directory =
            std::env::temp_dir().join(format!("lexicon-go-legacy-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let binary = directory.join(format!("lexicon-go{}", std::env::consts::EXE_SUFFIX));
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("adapters/go");
        let output = Command::new("go")
            .args(["build", "-o"])
            .arg(&binary)
            .arg(".")
            .current_dir(source)
            .output()
            .expect("build legacy Go adapter");
        assert!(
            output.status.success(),
            "legacy Go adapter build failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        binary
    })
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
        .join("adapters/go/testdata/oracle")
        .join(name)
}

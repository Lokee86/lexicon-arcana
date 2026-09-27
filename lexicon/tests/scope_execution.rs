mod support;

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::time::Duration;

use lexicon::{
    AnalysisPlan, ExecutionBudget, build_analysis_scope, execution_plan_with_limits,
    logical_shard_count,
};

use support::TestDirectory;

#[test]
fn scope_copies_selected_source_and_language_configuration() {
    let source = TestDirectory::new("scope-python-source");
    let temporary = TestDirectory::new("scope-python-temp");
    write(&source.path, "a.py", "value = 1\n");
    write(&source.path, "b.py", "value = 2\n");
    write(
        &source.path,
        "pyproject.toml",
        "[project]\nname='example'\n",
    );

    let repository =
        build_analysis_scope(&source.path, &temporary.path, "python", &["a.py".into()]).unwrap();
    assert!(repository.join("a.py").is_file());
    assert!(repository.join("pyproject.toml").is_file());
    assert!(!repository.join("b.py").exists());
}

#[test]
fn scope_expands_go_packages_and_rust_crates() {
    let go = TestDirectory::new("scope-go-source");
    let temporary = TestDirectory::new("scope-go-temp");
    write(&go.path, "go.mod", "module example.com/test\n");
    write(&go.path, "pkg/a.go", "package pkg\n");
    write(&go.path, "pkg/b.go", "package pkg\n");
    write(&go.path, "other/c.go", "package other\n");
    write(&go.path, "nested/go.mod", "module example.com/nested\n");
    write(&go.path, "nested/d.go", "package nested\n");
    write(&go.path, "vendor/ignored.go", "package ignored\n");
    write(&go.path, ".lexicon/ignored.go", "package ignored\n");
    let repository =
        build_analysis_scope(&go.path, &temporary.path, "go", &["pkg/a.go".into()]).unwrap();
    assert!(repository.join("pkg/a.go").is_file());
    assert!(repository.join("pkg/b.go").is_file());
    assert!(!repository.join("other/c.go").exists());
    assert!(repository.join("go.mod").is_file());
    assert!(repository.join("nested/go.mod").is_file());
    assert!(!repository.join("nested/d.go").exists());
    assert!(!repository.join("vendor/ignored.go").exists());
    assert!(!repository.join(".lexicon/ignored.go").exists());

    let rust = TestDirectory::new("scope-rust-source");
    write(&rust.path, "Cargo.toml", "[package]\nname='root'\n");
    write(&rust.path, "src/lib.rs", "mod child;\n");
    write(&rust.path, "src/child.rs", "pub fn child() {}\n");
    write(&rust.path, "other.rs", "pub fn outside() {}\n");
    let repository =
        build_analysis_scope(&rust.path, &temporary.path, "rust", &["src/lib.rs".into()]).unwrap();
    assert!(repository.join("src/lib.rs").is_file());
    assert!(repository.join("src/child.rs").is_file());
    assert!(repository.join("other.rs").is_file());
    assert!(repository.join("Cargo.toml").is_file());
}

#[test]
fn execution_plan_matches_go_partitioning_vectors() {
    let source = TestDirectory::new("execution-source");
    for index in 0..160 {
        write(
            &source.path,
            &format!("file_{index:03}.py"),
            "package sample\n",
        );
    }
    let plan = full_plan("python");
    let execution = execution_plan_with_limits(&source.path, &plan, 4, Some(3)).unwrap();
    assert_eq!(execution.logical_shards, 4);
    assert_eq!(execution.active_workers, 2);
    assert_eq!(execution.merge_fan_in, 2);
    assert_eq!(execution.source_files, 160);

    for index in 160..257 {
        write(
            &source.path,
            &format!("file_{index:03}.py"),
            "package sample\n",
        );
    }
    let execution = execution_plan_with_limits(&source.path, &plan, 16, None).unwrap();
    assert_eq!(execution.logical_shards, 8);
    assert_eq!(execution.active_workers, 4);
    assert_eq!(execution.merge_fan_in, 4);

    assert_eq!(logical_shard_count(536, 0), 16);
    assert_eq!(logical_shard_count(10_000, 0), 512);
    assert_eq!(logical_shard_count(250_000, 0), 4096);
}

#[test]
fn unsupported_adapter_skips_inventory_and_incremental_inventory_is_deduplicated() {
    let missing = PathBuf::from("definitely-missing-source-root");
    let ruby = execution_plan_with_limits(&missing, &full_plan("ruby"), 8, None).unwrap();
    assert_eq!(ruby.logical_shards, 1);
    assert_eq!(ruby.active_workers, 1);
    assert_eq!(ruby.source_files, 0);
    assert_eq!(ruby.source_bytes, 0);

    let source = TestDirectory::new("execution-incremental");
    write(&source.path, "a.py", "a");
    write(&source.path, "b.py", "bb");
    let plan = AnalysisPlan {
        language: "python".into(),
        full: false,
        known_present: false,
        changed_files: vec!["a.py".into(), "missing.py".into()],
        added_files: Vec::new(),
        removed_files: Vec::new(),
        context_files: vec!["a.py".into(), "b.py".into()],
    };
    let execution = execution_plan_with_limits(&source.path, &plan, 4, None).unwrap();
    assert_eq!(execution.source_files, 2);
    assert_eq!(execution.source_bytes, 3);
}

#[test]
fn weighted_budget_blocks_until_capacity_is_released() {
    let budget = Arc::new(ExecutionBudget::new(2));
    let held = budget.acquire(2);
    let barrier = Arc::new(Barrier::new(2));
    let other_budget = Arc::clone(&budget);
    let other_barrier = Arc::clone(&barrier);
    let handle = std::thread::spawn(move || {
        other_barrier.wait();
        let _permit = other_budget.acquire(1);
        true
    });
    barrier.wait();
    std::thread::sleep(Duration::from_millis(20));
    assert!(!handle.is_finished());
    drop(held);
    assert!(handle.join().unwrap());

    assert_eq!(ExecutionBudget::new(0).capacity(), 1);
}

fn full_plan(language: &str) -> AnalysisPlan {
    AnalysisPlan {
        language: language.into(),
        full: true,
        known_present: false,
        changed_files: Vec::new(),
        added_files: Vec::new(),
        removed_files: Vec::new(),
        context_files: Vec::new(),
    }
}

fn write(root: &std::path::Path, relative: &str, data: &str) {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}

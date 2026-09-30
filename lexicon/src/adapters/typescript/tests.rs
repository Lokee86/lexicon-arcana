use std::{ffi::OsString, path::PathBuf};

use crate::{AdapterHost, AdapterMode, AdapterRequest};

use super::TypeScriptAdapter;

#[test]
fn default_host_registers_typescript() {
    let host = AdapterHost::new(PathBuf::from("adapters"));
    assert!(host.has_adapter("typescript"));
}

#[test]
fn incremental_arguments_forward_changed_and_removed_paths() {
    let adapter = TypeScriptAdapter {
        node: OsString::from("node"),
        entrypoint: PathBuf::from("adapter/dist/cli.js"),
    };
    let request = AdapterRequest {
        language: "typescript".into(),
        mode: AdapterMode::Incremental,
        repository: PathBuf::from("repo"),
        changed_files: vec!["src\\changed.ts".into()],
        removed_files: vec!["src\\removed.tsx".into()],
        ..AdapterRequest::default()
    };
    let arguments = adapter
        .command_arguments(&request)
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        arguments,
        vec![
            "adapter/dist/cli.js",
            "--repo",
            "repo",
            "--output",
            "-",
            "--changed-file",
            "src/changed.ts",
            "--removed-file",
            "src/removed.tsx",
        ]
    );
}

#[test]
fn full_arguments_do_not_forward_incremental_scope() {
    let adapter = TypeScriptAdapter {
        node: OsString::from("node"),
        entrypoint: PathBuf::from("adapter/dist/cli.js"),
    };
    let request = AdapterRequest {
        language: "typescript".into(),
        mode: AdapterMode::Full,
        repository: PathBuf::from("repo"),
        changed_files: vec!["src/ignored.ts".into()],
        removed_files: vec!["src/ignored-too.ts".into()],
        ..AdapterRequest::default()
    };
    let arguments = adapter
        .command_arguments(&request)
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        arguments,
        vec!["adapter/dist/cli.js", "--repo", "repo", "--output", "-"]
    );
}

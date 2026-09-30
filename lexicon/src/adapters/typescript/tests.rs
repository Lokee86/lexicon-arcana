use std::{ffi::OsString, path::PathBuf};

use crate::{AdapterHost, AdapterMode, AdapterRequest, FactRecord, ValidationError};

use super::{TypeScriptAdapter, decode_output};

#[test]
fn default_host_registers_typescript() {
    let host = AdapterHost::new(PathBuf::from("adapters"));
    assert!(host.has_adapter("typescript"));
}

#[test]
fn bridge_defers_canonical_order_to_rust_host() {
    let source = format!("sha256:{}", "a".repeat(64));
    let target = format!("sha256:{}", "b".repeat(64));
    let records = [
        serde_json::json!({
            "adapter_version": "0.5.0",
            "language": "typescript",
            "record": "lexicon",
            "repository": "repo",
            "schema_version": 1
        }),
        serde_json::json!({
            "id": source,
            "kind": "function",
            "name": "source",
            "path": "src/a.ts",
            "qualified_name": "source",
            "record": "node"
        }),
        serde_json::json!({
            "id": target,
            "kind": "function",
            "name": "target",
            "path": "src/a.ts",
            "qualified_name": "target",
            "record": "node"
        }),
        serde_json::json!({
            "record": "edge",
            "relation": "calls",
            "source": source,
            "target": target
        }),
        serde_json::json!({
            "owner": "src/a.ts",
            "record": "edge",
            "relation": "calls",
            "source": source,
            "target": target
        }),
    ];
    let input = records
        .iter()
        .map(serde_json::Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";

    let mut analysis = decode_output(&input).expect("bridge should decode noncanonical output");
    assert_eq!(analysis.validate(), Err(ValidationError::NonCanonicalOrder));

    analysis.canonicalize().unwrap();
    analysis.validate().unwrap();
    let edges = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Edge(edge) => Some(edge),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0].owner.as_deref(), Some("src/a.ts"));
    assert_eq!(edges[1].owner, None);
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

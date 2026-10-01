use std::fs;

use serde_json::json;

use crate::adapters::c_family::clang_protocol::StructuralResponse;

use super::super::materialize;
use super::support::{TestDirectory, span};

#[test]
fn compiler_diagnostics_mark_structural_file_parse_error() {
    let root = TestDirectory::new("diagnostic");
    fs::write(root.path.join("bad.c"), b"int broken(;\n").unwrap();
    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 3,
        "helper_version": "0.7.0",
        "clang_version": "clang test",
        "compilation_database": false,
        "files": [{
            "path": "bad.c",
            "languages": ["c"],
            "diagnostics": [{
                "severity": "error",
                "message": "expected parameter declarator",
                "path": "bad.c",
                "span": span("bad.c", 1, 11, 1, 12)
            }]
        }]
    }))
    .unwrap();

    let model = materialize(&root.path, &response).unwrap();
    assert_eq!(model.files[0].language, "c");
    assert!(model.files[0].parse_error);
}

#[test]
fn rejects_noncanonical_observation_paths() {
    let root = TestDirectory::new("path");
    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 3,
        "helper_version": "0.7.0",
        "clang_version": "clang test",
        "compilation_database": false,
        "files": [{"path": "../escape.c", "languages": ["c"]}]
    }))
    .unwrap();

    let error = materialize(&root.path, &response).unwrap_err().to_string();
    assert!(error.contains("escapes repository"), "{error}");
}

use std::fs;

use serde_json::json;

use crate::adapters::c_family::{clang_protocol::StructuralResponse, facts};

use super::{
    super::materialize,
    support::{TestDirectory, span},
    value_flow_support::{declaration_ids, edge, function, parameter, symbol, variable},
};

#[test]
fn clang_bound_values_drive_accesses_and_passes_to() {
    let root = TestDirectory::new("semantic-dataflow");
    fs::write(
        root.path.join("flow.c"),
        b"void callee(int value){} void caller(void){ int local = 1; callee(local); }\n",
    )
    .unwrap();

    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 3,
        "helper_version": "0.7.0",
        "clang_version": "clang test",
        "compilation_database": true,
        "files": [{
            "path": "flow.c",
            "languages": ["c"],
            "translation_units": ["flow.c"],
            "declarations": [
                function("flow.c", "callee", "callee", 1, 1),
                parameter("flow.c", "value", "value", "callee", 0, false, 8),
                function("flow.c", "caller", "caller", 0, 15),
                variable("flow.c", "local", "local", "caller", false, 22)
            ],
            "calls": [{
                "source_compiler_id": "caller",
                "form": "direct",
                "resolution": "resolved",
                "expression": "callee",
                "target": symbol("callee", "flow.c", "callee", "Function", false),
                "candidates": [],
                "receiver_type_name": "",
                "virtual_dispatch": false,
                "compiler_candidate_count": 1,
                "arguments": [{
                    "expression": "local",
                    "value": symbol("local", "flow.c", "caller::local", "Var", false)
                }],
                "span": span("flow.c", 1, 58, 1, 71)
            }],
            "accesses": [
                {
                    "source_compiler_id": "caller",
                    "target": symbol("local", "flow.c", "caller::local", "Var", false),
                    "relation": "writes",
                    "expression": "local",
                    "span": span("flow.c", 1, 47, 1, 52)
                },
                {
                    "source_compiler_id": "caller",
                    "target": symbol("local", "flow.c", "caller::local", "Var", false),
                    "relation": "reads",
                    "expression": "local",
                    "span": span("flow.c", 1, 65, 1, 70)
                }
            ]
        }]
    }))
    .unwrap();

    let model = materialize(&root.path, &response).unwrap();
    let ids = declaration_ids(&model.files[0]);
    let analysis = facts::analysis(&Default::default(), model);

    assert!(edge(
        &analysis.records,
        &ids["caller"],
        &ids["caller::local"],
        "writes"
    ));
    assert!(edge(
        &analysis.records,
        &ids["caller"],
        &ids["caller::local"],
        "reads"
    ));
    assert!(edge(
        &analysis.records,
        &ids["caller::local"],
        &ids["callee::value"],
        "passes-to"
    ));
}

#[test]
fn context_identity_cannot_be_a_materialized_pointer_binding_source() {
    let root = TestDirectory::new("context-pointer-source");
    fs::write(root.path.join("owner.c"), b"void owner(void) {}\n").unwrap();

    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 3,
        "helper_version": "0.7.0",
        "clang_version": "clang test",
        "compilation_database": true,
        "context_identities": [{
            "compiler_id": "context-pointer-usr",
            "path": "context.h",
            "kind": "variable",
            "qualified_name": "context_pointer",
            "signature": "",
            "definition": true
        }],
        "files": [{
            "path": "owner.c",
            "languages": ["c"],
            "translation_units": ["owner.c"],
            "declarations": [
                function("owner.c", "owner", "owner", 0, 1)
            ],
            "pointer_bindings": [{
                "pointer": symbol(
                    "context-pointer-usr",
                    "context.h",
                    "context_pointer",
                    "Var",
                    false
                ),
                "target": symbol("external-target", "", "target", "Function", true),
                "expression": "context_pointer = target",
                "span": span("owner.c", 1, 1, 1, 19)
            }]
        }]
    }))
    .unwrap();

    let error = materialize(&root.path, &response).unwrap_err().to_string();
    assert!(
        error.contains("pointer \"context_pointer\" is not materialized"),
        "{error}"
    );
}

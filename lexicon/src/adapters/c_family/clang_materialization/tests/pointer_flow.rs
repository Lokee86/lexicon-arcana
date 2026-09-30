use std::fs;

use serde_json::json;

use crate::{
    FactRecord,
    adapters::c_family::{clang_protocol::StructuralResponse, facts},
};

use super::{
    super::materialize,
    support::{TestDirectory, span},
    value_flow_support::{declaration_ids, edge, function, parameter, symbol, variable},
};

#[test]
fn clang_pointer_and_callback_flow_drive_indirect_calls() {
    let root = TestDirectory::new("semantic-pointer-flow");
    fs::write(
        root.path.join("callbacks.c"),
        b"void callback(void){} void dispatch(void (*cb)(void)){ cb(); } void caller(void){ void (*fp)(void)=callback; dispatch(fp); fp(); }\n",
    )
    .unwrap();

    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 1,
        "helper_version": "0.4.0",
        "clang_version": "clang test",
        "compilation_database": true,
        "files": [{
            "path": "callbacks.c",
            "languages": ["c"],
            "translation_units": ["callbacks.c"],
            "declarations": [
                function("callbacks.c", "callback", "callback", 0, 1),
                function("callbacks.c", "dispatch", "dispatch", 1, 10),
                parameter("callbacks.c", "cb", "cb", "dispatch", 0, true, 20),
                function("callbacks.c", "caller", "caller", 0, 30),
                variable("callbacks.c", "fp", "fp", "caller", true, 40)
            ],
            "pointer_bindings": [{
                "pointer": symbol("fp", "callbacks.c", "caller::fp", "Var", false),
                "target": symbol("callback", "callbacks.c", "callback", "Function", false),
                "expression": "fp = callback",
                "span": span("callbacks.c", 1, 80, 1, 93)
            }],
            "calls": [
                {
                    "source_compiler_id": "caller",
                    "form": "direct",
                    "resolution": "resolved",
                    "expression": "dispatch",
                    "target": symbol("dispatch", "callbacks.c", "dispatch", "Function", false),
                    "candidates": [],
                    "receiver_type_name": "",
                    "virtual_dispatch": false,
                    "compiler_candidate_count": 1,
                    "arguments": [{
                        "expression": "fp",
                        "value": symbol("fp", "callbacks.c", "caller::fp", "Var", false)
                    }],
                    "span": span("callbacks.c", 1, 95, 1, 107)
                },
                {
                    "source_compiler_id": "dispatch",
                    "form": "direct",
                    "resolution": "indirect",
                    "expression": "cb",
                    "candidates": [],
                    "callee_value": symbol("cb", "callbacks.c", "dispatch::cb", "ParmVar", false),
                    "receiver_type_name": "",
                    "virtual_dispatch": false,
                    "compiler_candidate_count": 0,
                    "arguments": [],
                    "span": span("callbacks.c", 1, 55, 1, 59)
                },
                {
                    "source_compiler_id": "caller",
                    "form": "direct",
                    "resolution": "indirect",
                    "expression": "fp",
                    "candidates": [],
                    "callee_value": symbol("fp", "callbacks.c", "caller::fp", "Var", false),
                    "receiver_type_name": "",
                    "virtual_dispatch": false,
                    "macro_expanded": true,
                    "compiler_candidate_count": 0,
                    "arguments": [],
                    "span": span("callbacks.c", 1, 109, 1, 113)
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
        &ids["caller::fp"],
        &ids["dispatch::cb"],
        "passes-to"
    ));
    assert!(edge(
        &analysis.records,
        &ids["dispatch"],
        &ids["callback"],
        "possible-calls"
    ));
    assert!(edge(
        &analysis.records,
        &ids["caller"],
        &ids["callback"],
        "possible-calls"
    ));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(value)
            if value.source == ids["caller"]
                && value.target == ids["callback"]
                && value.attributes.as_ref().is_some_and(|attributes| {
                    let text = attributes.to_string();
                    text.contains("clang-macro-expansion")
                        && text.contains("function-pointer")
                })
    )));
    assert!(
        analysis
            .records
            .iter()
            .filter(|record| matches!(
                record,
                FactRecord::Unresolved(value) if value.reason == "dynamic-target"
            ))
            .count()
            >= 2
    );
}

#[test]
fn clang_external_pointer_binding_does_not_require_repository_materialization() {
    let root = TestDirectory::new("semantic-external-pointer-flow");
    fs::write(root.path.join("signal.c"), b"void callback(void){}\n").unwrap();

    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 1,
        "helper_version": "0.4.0",
        "clang_version": "clang test",
        "compilation_database": true,
        "files": [{
            "path": "signal.c",
            "languages": ["c"],
            "translation_units": ["signal.c"],
            "declarations": [
                function("signal.c", "callback", "callback", 0, 1)
            ],
            "pointer_bindings": [{
                "pointer": symbol(
                    "external-sa-handler",
                    "",
                    "sigaction::(anonymous union)::sa_handler",
                    "Field",
                    true
                ),
                "target": symbol("callback", "signal.c", "callback", "Function", false),
                "expression": "action.sa_handler = callback",
                "span": span("signal.c", 1, 1, 1, 30)
            }]
        }]
    }))
    .unwrap();

    let model = materialize(&root.path, &response).unwrap();

    assert!(model.files[0].semantic_pointer_bindings.is_empty());
}

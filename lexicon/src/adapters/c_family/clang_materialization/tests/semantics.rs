use std::fs;

use serde_json::json;

use crate::FactRecord;
use crate::adapters::c_family::{clang_protocol::StructuralResponse, facts};

use super::super::materialize;
use super::support::{TestDirectory, span};

#[test]
fn clang_semantics_materialize_relationships_and_call_policy() {
    let root = TestDirectory::new("semantic-relationships");
    fs::write(
        root.path.join("semantic.cpp"),
        b"struct Base { virtual int value(); }; struct Derived : Base { int value() override; }; int foo(int); int foo(double); int caller();\n",
    )
    .unwrap();

    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 1,
        "helper_version": "0.4.0",
        "clang_version": "clang test",
        "compilation_database": true,
        "files": [{
            "path": "semantic.cpp",
            "languages": ["cpp"],
            "translation_units": ["semantic.cpp"],
            "declarations": [
                declaration("base", "type", "Base", "Base", "", "", "struct", 1),
                declaration("base-value", "method", "value", "Base::value", "value()", "int", "", 5),
                declaration("derived", "type", "Derived", "Derived", "", "", "struct", 12),
                declaration("derived-value", "method", "value", "Derived::value", "value()", "int", "", 18),
                declaration("foo-int", "function", "foo", "foo", "foo(int value)", "int", "", 25),
                declaration("foo-double", "function", "foo", "foo", "foo(double value)", "int", "", 31),
                declaration("caller", "function", "caller", "caller", "caller()", "int", "", 38)
            ],
            "relationships": [
                {
                    "kind": "extends",
                    "source_compiler_id": "derived",
                    "target": symbol("base", "semantic.cpp", "Base", "CXXRecord", false),
                    "expression": "Base",
                    "span": span("semantic.cpp", 1, 55, 1, 59)
                },
                {
                    "kind": "overrides",
                    "source_compiler_id": "derived-value",
                    "target": symbol("base-value", "semantic.cpp", "Base::value", "CXXMethod", false),
                    "expression": "Derived::value",
                    "span": span("semantic.cpp", 1, 70, 1, 75)
                }
            ],
            "calls": [
                {
                    "source_compiler_id": "caller",
                    "form": "direct",
                    "resolution": "resolved",
                    "expression": "foo",
                    "target": symbol("foo-int", "semantic.cpp", "foo", "Function", false),
                    "candidates": [],
                    "receiver_type_name": "",
                    "virtual_dispatch": false,
                    "compiler_candidate_count": 1,
                    "arguments": [{"expression": "1"}],
                    "span": span("semantic.cpp", 2, 1, 2, 6)
                },
                {
                    "source_compiler_id": "caller",
                    "form": "direct",
                    "resolution": "ambiguous",
                    "expression": "foo",
                    "candidates": [
                        symbol("foo-int", "semantic.cpp", "foo", "Function", false),
                        symbol("foo-double", "semantic.cpp", "foo", "Function", false)
                    ],
                    "receiver_type_name": "",
                    "virtual_dispatch": false,
                    "compiler_candidate_count": 2,
                    "arguments": [{"expression": "value"}],
                    "span": span("semantic.cpp", 3, 1, 3, 10)
                },
                {
                    "source_compiler_id": "caller",
                    "form": "member",
                    "resolution": "resolved",
                    "expression": "base.value",
                    "target": symbol("base-value", "semantic.cpp", "Base::value", "CXXMethod", false),
                    "candidates": [],
                    "receiver_type": symbol("base", "semantic.cpp", "Base", "CXXRecord", false),
                    "receiver_type_name": "Base",
                    "virtual_dispatch": true,
                    "compiler_candidate_count": 1,
                    "arguments": [],
                    "span": span("semantic.cpp", 4, 1, 4, 13)
                }
            ]
        }]
    }))
    .unwrap();

    let model = materialize(&root.path, &response).unwrap();
    let file = &model.files[0];
    assert_eq!(file.semantic_relationships.len(), 2);
    assert_eq!(file.semantic_calls.len(), 3);

    let ids = file
        .declarations
        .iter()
        .map(|declaration| (declaration.qualified_name.clone(), declaration.id.clone()))
        .collect::<std::collections::HashMap<_, _>>();
    let caller = ids["caller"].clone();
    let base = ids["Base"].clone();
    let derived = ids["Derived"].clone();
    let base_value = ids["Base::value"].clone();
    let derived_value = ids["Derived::value"].clone();

    let analysis = facts::analysis(&Default::default(), model);
    assert!(edge(&analysis.records, &derived, &base, "extends"));
    assert!(edge(
        &analysis.records,
        &derived_value,
        &base_value,
        "overrides"
    ));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.source == caller
                && edge.relation == "calls"
                && edge.attributes.as_ref().and_then(|value| value.get("evidence"))
                    .is_some_and(|value| value.to_string().contains("clang-direct"))
    )));
    assert!(edge(
        &analysis.records,
        &caller,
        &base_value,
        "possible-calls"
    ));
    assert!(edge(
        &analysis.records,
        &caller,
        &derived_value,
        "possible-calls"
    ));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Unresolved(value)
            if value.source == caller && value.reason == "ambiguous-target"
    )));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Unresolved(value)
            if value.source == caller && value.reason == "dynamic-target"
    )));
}

#[test]
fn macro_spanned_semantics_resolve_source_identity_across_files() {
    let root = TestDirectory::new("semantic-cross-file-source");
    fs::write(
        root.path.join("caller.cpp"),
        b"#include \"macro.h\"\nint caller() { return WRAP(); }\n",
    )
    .unwrap();
    fs::write(root.path.join("macro.h"), b"#define WRAP() target\n").unwrap();

    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 1,
        "helper_version": "0.4.0",
        "clang_version": "clang test",
        "compilation_database": true,
        "files": [
            {
                "path": "caller.cpp",
                "languages": ["cpp"],
                "translation_units": ["caller.cpp"],
                "declarations": [{
                    "compiler_id": "caller-usr",
                    "kind": "function",
                    "name": "caller",
                    "qualified_name": "caller",
                    "signature": "caller()",
                    "span": span("caller.cpp", 2, 1, 2, 13),
                    "callable": true,
                    "definition": true,
                    "internal": false,
                    "template": false,
                    "virtual_member": false,
                    "function_pointer": false,
                    "alias": false,
                    "enum_member": false
                }]
            },
            {
                "path": "macro.h",
                "languages": ["cpp"],
                "translation_units": ["caller.cpp"],
                "declarations": [{
                    "compiler_id": "target-usr",
                    "kind": "variable",
                    "name": "target",
                    "qualified_name": "target",
                    "type_name": "int",
                    "span": span("macro.h", 1, 16, 1, 22),
                    "callable": false,
                    "definition": true,
                    "internal": false,
                    "template": false,
                    "virtual_member": false,
                    "function_pointer": false,
                    "alias": false,
                    "enum_member": false
                }],
                "calls": [{
                    "source_compiler_id": "caller-usr",
                    "form": "direct",
                    "resolution": "missing",
                    "expression": "WRAP",
                    "candidates": [],
                    "receiver_type_name": "",
                    "virtual_dispatch": false,
                    "macro_expanded": true,
                    "compiler_candidate_count": 0,
                    "arguments": [],
                    "span": span("macro.h", 1, 1, 1, 8)
                }],
                "accesses": [{
                    "source_compiler_id": "caller-usr",
                    "target": symbol("target-usr", "macro.h", "target", "Var", false),
                    "relation": "reads",
                    "expression": "target",
                    "span": span("macro.h", 1, 16, 1, 22)
                }]
            }
        ]
    }))
    .unwrap();

    let model = materialize(&root.path, &response).unwrap();
    let caller_id = model
        .files
        .iter()
        .find(|file| file.path == "caller.cpp")
        .unwrap()
        .declarations[0]
        .id
        .clone();
    let header = model
        .files
        .iter()
        .find(|file| file.path == "macro.h")
        .unwrap();
    assert_eq!(header.semantic_calls[0].source_id, caller_id);
    assert_eq!(header.semantic_accesses[0].source_id, caller_id);
}

#[test]
fn resolved_external_clang_target_stays_unresolved_in_lexicon_policy() {
    let root = TestDirectory::new("semantic-external");
    fs::write(
        root.path.join("main.c"),
        b"int caller(void) { return puts(\"x\"); }\n",
    )
    .unwrap();
    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 1,
        "helper_version": "0.4.0",
        "clang_version": "clang test",
        "compilation_database": true,
        "files": [{
            "path": "main.c",
            "languages": ["c"],
            "translation_units": ["main.c"],
            "declarations": [
                declaration("caller", "function", "caller", "caller", "caller(void)", "int", "", 1)
            ],
            "calls": [{
                "source_compiler_id": "caller",
                "form": "direct",
                "resolution": "resolved",
                "expression": "puts",
                "target": symbol("libc-puts", "", "puts", "Function", true),
                "candidates": [],
                "receiver_type_name": "",
                "virtual_dispatch": false,
                "compiler_candidate_count": 1,
                "arguments": [{"expression": "\"x\""}],
                "span": span("main.c", 1, 27, 1, 36)
            }]
        }]
    }))
    .unwrap();

    let model = materialize(&root.path, &response).unwrap();
    let caller = model.files[0].declarations[0].id.clone();
    let analysis = facts::analysis(&Default::default(), model);
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Unresolved(value)
            if value.source == caller && value.reason == "external-target"
    )));
}

#[test]
fn clang_entity_prefers_repository_definition_when_callsite_has_no_same_file_redeclaration() {
    let root = TestDirectory::new("semantic-definition-owner");
    fs::write(root.path.join("api.h"), b"int target(int value);\n").unwrap();
    fs::write(
        root.path.join("impl.cpp"),
        b"int target(int value) { return value; }\n",
    )
    .unwrap();
    fs::write(
        root.path.join("caller.cpp"),
        b"#include \"api.h\"\nint caller() { return target(1); }\n",
    )
    .unwrap();

    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 1,
        "helper_version": "0.4.0",
        "clang_version": "clang test",
        "compilation_database": true,
        "files": [
            {
                "path": "api.h",
                "languages": ["cpp"],
                "translation_units": ["caller.cpp"],
                "declarations": [{
                    "compiler_id": "target-usr",
                    "kind": "function",
                    "name": "target",
                    "qualified_name": "target",
                    "signature": "target(int value)",
                    "span": span("api.h", 1, 1, 1, 22),
                    "callable": true,
                    "definition": false,
                    "internal": false,
                    "template": false,
                    "virtual_member": false,
                    "function_pointer": false,
                    "alias": false,
                    "enum_member": false
                }]
            },
            {
                "path": "caller.cpp",
                "languages": ["cpp"],
                "translation_units": ["caller.cpp"],
                "declarations": [{
                    "compiler_id": "caller-usr",
                    "kind": "function",
                    "name": "caller",
                    "qualified_name": "caller",
                    "signature": "caller()",
                    "span": span("caller.cpp", 2, 1, 2, 13),
                    "callable": true,
                    "definition": true,
                    "internal": false,
                    "template": false,
                    "virtual_member": false,
                    "function_pointer": false,
                    "alias": false,
                    "enum_member": false
                }],
                "calls": [{
                    "source_compiler_id": "caller-usr",
                    "form": "direct",
                    "resolution": "resolved",
                    "expression": "target",
                    "target": symbol("target-usr", "api.h", "target", "Function", false),
                    "candidates": [],
                    "receiver_type_name": "",
                    "virtual_dispatch": false,
                    "compiler_candidate_count": 1,
                    "arguments": [{"expression": "1"}],
                    "span": span("caller.cpp", 2, 23, 2, 32)
                }]
            },
            {
                "path": "impl.cpp",
                "languages": ["cpp"],
                "translation_units": ["impl.cpp"],
                "declarations": [{
                    "compiler_id": "target-usr",
                    "kind": "function",
                    "name": "target",
                    "qualified_name": "target",
                    "signature": "target(int value)",
                    "span": span("impl.cpp", 1, 1, 1, 38),
                    "callable": true,
                    "definition": true,
                    "internal": false,
                    "template": false,
                    "virtual_member": false,
                    "function_pointer": false,
                    "alias": false,
                    "enum_member": false
                }]
            }
        ]
    }))
    .unwrap();

    let model = materialize(&root.path, &response).unwrap();
    let caller = model
        .files
        .iter()
        .find(|file| file.path == "caller.cpp")
        .unwrap();
    let target = model
        .files
        .iter()
        .find(|file| file.path == "impl.cpp")
        .unwrap()
        .declarations[0]
        .id
        .clone();
    assert_eq!(caller.semantic_calls[0].target_id, target);
}

fn declaration(
    compiler_id: &str,
    kind: &str,
    name: &str,
    qualified_name: &str,
    signature: &str,
    type_name: &str,
    tag: &str,
    column: u64,
) -> serde_json::Value {
    json!({
        "compiler_id": compiler_id,
        "kind": kind,
        "name": name,
        "qualified_name": qualified_name,
        "signature": signature,
        "type_name": type_name,
        "tag": tag,
        "span": span("semantic.cpp", 1, column, 1, column + 4),
        "callable": matches!(kind, "function" | "method" | "constructor"),
        "definition": true,
        "internal": false,
        "template": false,
        "virtual_member": qualified_name.ends_with("value"),
        "function_pointer": false,
        "alias": false,
        "enum_member": false
    })
}

fn symbol(
    compiler_id: &str,
    path: &str,
    qualified_name: &str,
    kind: &str,
    external: bool,
) -> serde_json::Value {
    json!({
        "compiler_id": compiler_id,
        "path": path,
        "qualified_name": qualified_name,
        "kind": kind,
        "external": external
    })
}

fn edge(records: &[FactRecord], source: &str, target: &str, relation: &str) -> bool {
    records.iter().any(|record| {
        matches!(
            record,
            FactRecord::Edge(edge)
                if edge.source == source && edge.target == target && edge.relation == relation
        )
    })
}

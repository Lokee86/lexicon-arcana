use std::fs;

use serde_json::json;

use crate::adapters::c_family::{clang_protocol::StructuralResponse, facts};
use crate::{FactRecord, node_id};

use super::super::materialize;
use super::support::{TestDirectory, span};

#[test]
fn materializes_clang_structural_observations_into_lexicon_identities() {
    let root = TestDirectory::new("materialize");
    fs::write(
        root.path.join("main.cpp"),
        b"#include \"thing.hpp\"\n#define WRAP(x) target(x)\nnamespace demo { struct Thing { int field; }; int run(int value); }\n",
    )
    .unwrap();
    fs::write(root.path.join("thing.hpp"), b"struct HeaderType {};\n").unwrap();

    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 3,
        "helper_version": "0.7.0",
        "clang_version": "clang test",
        "compilation_database": true,
        "translation_units": [{
            "path": "main.cpp",
            "language": "cpp",
            "directory": ".",
            "arguments": ["clang++", "-std=c++20", "main.cpp"],
            "synthesized": false
        }],
        "files": [
            {
                "path": "thing.hpp",
                "languages": ["cpp"],
                "translation_units": ["main.cpp"]
            },
            {
                "path": "main.cpp",
                "languages": ["cpp"],
                "translation_units": ["main.cpp"],
                "declarations": [
                    {
                        "compiler_id": "usr:ns",
                        "kind": "namespace",
                        "name": "demo",
                        "qualified_name": "demo",
                        "span": span("main.cpp", 3, 1, 3, 75),
                        "callable": false,
                        "definition": true,
                        "internal": false,
                        "template": false,
                        "virtual_member": false,
                        "function_pointer": false,
                        "alias": false,
                        "enum_member": false
                    },
                    {
                        "compiler_id": "usr:type",
                        "kind": "type",
                        "name": "Thing",
                        "qualified_name": "demo::Thing",
                        "tag": "struct",
                        "container_compiler_id": "usr:ns",
                        "span": span("main.cpp", 3, 18, 3, 44),
                        "callable": false,
                        "definition": true,
                        "internal": false,
                        "template": false,
                        "virtual_member": false,
                        "function_pointer": false,
                        "alias": false,
                        "enum_member": false
                    },
                    {
                        "compiler_id": "usr:field",
                        "kind": "field",
                        "name": "field",
                        "qualified_name": "demo::Thing::field",
                        "type_name": "int",
                        "container_compiler_id": "usr:type",
                        "parent_type_compiler_id": "usr:type",
                        "span": span("main.cpp", 3, 33, 3, 42),
                        "callable": false,
                        "definition": true,
                        "internal": false,
                        "template": false,
                        "virtual_member": false,
                        "function_pointer": false,
                        "alias": false,
                        "enum_member": false
                    },
                    {
                        "compiler_id": "usr:run",
                        "kind": "function",
                        "name": "run",
                        "qualified_name": "demo::run",
                        "signature": "run(int value)",
                        "container_compiler_id": "usr:ns",
                        "span": span("main.cpp", 3, 46, 3, 65),
                        "callable": true,
                        "definition": false,
                        "internal": false,
                        "template": false,
                        "virtual_member": false,
                        "function_pointer": false,
                        "alias": false,
                        "enum_member": false,
                        "parameter_count": 1
                    },
                    {
                        "compiler_id": "usr:param",
                        "kind": "parameter",
                        "name": "value",
                        "qualified_name": "demo::run::value",
                        "signature": "parameter#0",
                        "type_name": "int",
                        "container_compiler_id": "usr:run",
                        "span": span("main.cpp", 3, 54, 3, 63),
                        "callable": false,
                        "definition": true,
                        "internal": false,
                        "template": false,
                        "virtual_member": false,
                        "function_pointer": false,
                        "alias": false,
                        "enum_member": false,
                        "parameter_index": 0
                    }
                ],
                "includes": [{
                    "target": "virtual/thing.hpp",
                    "resolved_path": "thing.hpp",
                    "expression": "\"virtual/thing.hpp\"",
                    "system": false,
                    "offset": 0,
                    "span": span("main.cpp", 1, 1, 1, 21)
                }],
                "macros": [{
                    "compiler_id": "macro:main.cpp:21:WRAP",
                    "name": "WRAP",
                    "replacement": "target(x)",
                    "function_like": true,
                    "conditional": false,
                    "parameters": ["x"],
                    "offset": 21,
                    "span": span("main.cpp", 2, 1, 2, 26)
                }]
            }
        ]
    }))
    .unwrap();

    let model = materialize(&root.path, &response).unwrap();
    assert_eq!(
        model
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        ["main.cpp", "thing.hpp"]
    );
    let file = &model.files[0];
    assert_eq!(file.parser, "clang");
    assert_eq!(file.language, "cpp");
    assert!(!file.parse_error);

    let namespace = file
        .declarations
        .iter()
        .find(|value| value.name == "demo")
        .unwrap();
    let namespace_id = node_id("c-family", "namespace", "main.cpp::namespace::demo");
    assert_eq!(namespace.id, namespace_id);
    let ty = file
        .declarations
        .iter()
        .find(|value| value.qualified_name == "demo::Thing")
        .unwrap();
    assert_eq!(ty.container_id, namespace_id);
    assert_eq!(ty.attributes.get("tag"), Some(&json!("struct")));

    let run = file
        .declarations
        .iter()
        .find(|value| value.qualified_name == "demo::run")
        .unwrap();
    assert_eq!(run.attributes.get("parameter_count"), Some(&json!(1)));
    let run_id = node_id(
        "c-family",
        "function",
        "main.cpp::function::demo::run::run(int value)",
    );
    assert_eq!(run.id, run_id);
    let parameter = file
        .declarations
        .iter()
        .find(|value| value.kind == "parameter")
        .unwrap();
    assert_eq!(parameter.container_id, run_id);
    assert_eq!(parameter.attributes.get("index"), Some(&json!(0)));

    let macro_decl = file
        .declarations
        .iter()
        .find(|value| value.name == "WRAP")
        .unwrap();
    assert_eq!(macro_decl.attributes.get("macro"), Some(&json!(true)));
    assert_eq!(
        macro_decl.attributes.get("replacement"),
        Some(&json!("target(x)"))
    );
    assert!(!macro_decl.attributes.contains_key("target"));

    assert_eq!(file.includes.len(), 1);
    assert_eq!(file.includes[0].resolved_path, "thing.hpp");
    let import_id = node_id(
        "c-family",
        "import",
        "main.cpp::include::virtual/thing.hpp::0",
    );
    assert_eq!(file.includes[0].id, import_id);

    let analysis = facts::analysis(&Default::default(), model);
    let header_id = node_id("c-family", "file", "thing.hpp");
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Edge(edge)
            if edge.relation == "includes"
                && edge.source == import_id
                && edge.target == header_id
    )));
    assert!(analysis.records.iter().any(|record| matches!(
        record,
        FactRecord::Node(node)
            if node.kind == "file"
                && node.path == "main.cpp"
                && node.attributes.as_ref().and_then(|value| value.get("parser")) == Some(&json!("clang"))
    )));
}

#[test]
fn ambiguous_orphan_header_preserves_content_language_policy() {
    let root = TestDirectory::new("ambiguous-orphan-header");
    fs::write(
        root.path.join("orphan.h"),
        b"namespace demo { class Value {}; }\n",
    )
    .unwrap();
    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 3,
        "helper_version": "0.7.0",
        "clang_version": "clang test",
        "compilation_database": false,
        "files": [{
            "path": "orphan.h",
            "languages": ["c", "cpp"],
            "translation_units": ["orphan.h"]
        }]
    }))
    .unwrap();

    let model = materialize(&root.path, &response).unwrap();
    assert_eq!(model.files[0].language, "cpp");
}

#[test]
fn mixed_real_translation_unit_header_preserves_c_first_policy() {
    let root = TestDirectory::new("mixed-real-header");
    fs::write(
        root.path.join("shared.h"),
        b"namespace demo { class Value {}; }\n",
    )
    .unwrap();
    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 3,
        "helper_version": "0.7.0",
        "clang_version": "clang test",
        "compilation_database": true,
        "files": [{
            "path": "shared.h",
            "languages": ["c", "cpp"],
            "translation_units": ["main.c", "main.cpp"]
        }]
    }))
    .unwrap();

    let model = materialize(&root.path, &response).unwrap();
    assert_eq!(model.files[0].language, "c");
}

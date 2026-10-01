use std::fs;

use serde_json::json;

use crate::adapters::c_family::clang_protocol::StructuralResponse;

use super::super::materialize;
use super::support::{TestDirectory, span};

#[test]
fn preserves_frozen_c_family_callable_identity_vectors() {
    let root = TestDirectory::new("identity-vectors");
    fs::write(root.path.join("api.h"), b"int add(int left, int right);\n").unwrap();
    fs::write(
        root.path.join("types.cpp"),
        b"namespace demo { int Base::value(int input) const { return input; } }\n",
    )
    .unwrap();

    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 3,
        "helper_version": "0.7.0",
        "clang_version": "clang test",
        "compilation_database": true,
        "files": [
            {
                "path": "api.h",
                "languages": ["c"],
                "declarations": [{
                    "compiler_id": "usr:add",
                    "kind": "function",
                    "name": "add",
                    "qualified_name": "add",
                    "signature": "add(int left, int right)",
                    "span": span("api.h", 1, 1, 1, 30),
                    "callable": true,
                    "definition": false,
                    "internal": false,
                    "template": false,
                    "virtual_member": false,
                    "function_pointer": false,
                    "alias": false,
                    "enum_member": false,
                    "parameter_count": 2
                }]
            },
            {
                "path": "types.cpp",
                "languages": ["cpp"],
                "declarations": [{
                    "compiler_id": "usr:value-definition",
                    "kind": "function",
                    "name": "value",
                    "qualified_name": "demo::Base::value",
                    "signature": "Base::value(int input) const",
                    "span": span("types.cpp", 1, 18, 1, 66),
                    "callable": true,
                    "definition": true,
                    "internal": false,
                    "template": false,
                    "virtual_member": true,
                    "function_pointer": false,
                    "alias": false,
                    "enum_member": false,
                    "parameter_count": 1
                }]
            }
        ]
    }))
    .unwrap();

    let model = materialize(&root.path, &response).unwrap();
    let add = model.files[0]
        .declarations
        .iter()
        .find(|value| value.name == "add")
        .unwrap();
    assert_eq!(
        add.id,
        "sha256:b4bd5c075c598908c4bd01a28e8e708ef3880c83ee686e8fa3f7497674f771a1"
    );
    let value = model.files[1]
        .declarations
        .iter()
        .find(|declaration| declaration.name == "value")
        .unwrap();
    assert_eq!(value.kind, "function");
    assert_eq!(
        value.id,
        "sha256:fd85ab25e9d0dc88ffbfd42e64556b8b4dbb554810af0c555139e2ace0691a31"
    );
}

#[test]
fn compiler_ids_are_correlated_within_each_source_owner() {
    let root = TestDirectory::new("owner-local-identities");
    fs::write(
        root.path.join("left.cpp"),
        b"namespace demo { int value; }\n",
    )
    .unwrap();
    fs::write(
        root.path.join("right.cpp"),
        b"namespace demo { int value; }\n",
    )
    .unwrap();

    let declaration = |path: &str| {
        json!({
            "path": path,
            "languages": ["cpp"],
            "declarations": [
                {
                    "compiler_id": "usr:demo",
                    "kind": "namespace",
                    "name": "demo",
                    "qualified_name": "demo",
                    "span": span(path, 1, 1, 1, 30),
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
                    "compiler_id": "usr:value",
                    "kind": "variable",
                    "name": "value",
                    "qualified_name": "demo::value",
                    "type_name": "int",
                    "container_compiler_id": "usr:demo",
                    "span": span(path, 1, 18, 1, 27),
                    "callable": false,
                    "definition": true,
                    "internal": false,
                    "template": false,
                    "virtual_member": false,
                    "function_pointer": false,
                    "alias": false,
                    "enum_member": false
                }
            ]
        })
    };
    let response: StructuralResponse = serde_json::from_value(json!({
        "protocol_version": 3,
        "helper_version": "0.7.0",
        "clang_version": "clang test",
        "compilation_database": false,
        "files": [declaration("left.cpp"), declaration("right.cpp")]
    }))
    .unwrap();

    let model = materialize(&root.path, &response).unwrap();
    for file in &model.files {
        let namespace = file
            .declarations
            .iter()
            .find(|value| value.kind == "namespace")
            .unwrap();
        let value = file
            .declarations
            .iter()
            .find(|value| value.kind == "variable")
            .unwrap();
        assert_eq!(value.container_id, namespace.id);
    }
    assert_ne!(
        model.files[0].declarations[0].id,
        model.files[1].declarations[0].id
    );
}

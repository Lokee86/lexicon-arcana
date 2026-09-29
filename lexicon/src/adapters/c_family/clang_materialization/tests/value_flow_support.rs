use std::collections::HashMap;

use serde_json::json;

use crate::FactRecord;

use super::support::span;

pub(super) fn function(
    path: &str,
    compiler_id: &str,
    name: &str,
    parameter_count: usize,
    column: u64,
) -> serde_json::Value {
    json!({
        "compiler_id": compiler_id,
        "kind": "function",
        "name": name,
        "qualified_name": name,
        "signature": format!("{name}()"),
        "span": span(path, 1, column, 1, column + 4),
        "callable": true,
        "definition": true,
        "internal": false,
        "template": false,
        "virtual_member": false,
        "function_pointer": false,
        "alias": false,
        "enum_member": false,
        "parameter_count": parameter_count
    })
}

pub(super) fn parameter(
    path: &str,
    compiler_id: &str,
    name: &str,
    container: &str,
    index: usize,
    function_pointer: bool,
    column: u64,
) -> serde_json::Value {
    json!({
        "compiler_id": compiler_id,
        "kind": "parameter",
        "name": name,
        "qualified_name": format!("{container}::{name}"),
        "type_name": if function_pointer { "void (*)(void)" } else { "int" },
        "container_compiler_id": container,
        "span": span(path, 1, column, 1, column + 4),
        "callable": false,
        "definition": true,
        "internal": false,
        "template": false,
        "virtual_member": false,
        "function_pointer": function_pointer,
        "alias": false,
        "enum_member": false,
        "parameter_index": index
    })
}

pub(super) fn variable(
    path: &str,
    compiler_id: &str,
    name: &str,
    container: &str,
    function_pointer: bool,
    column: u64,
) -> serde_json::Value {
    json!({
        "compiler_id": compiler_id,
        "kind": "variable",
        "name": name,
        "qualified_name": format!("{container}::{name}"),
        "type_name": if function_pointer { "void (*)(void)" } else { "int" },
        "container_compiler_id": container,
        "span": span(path, 1, column, 1, column + 4),
        "callable": false,
        "definition": true,
        "internal": false,
        "template": false,
        "virtual_member": false,
        "function_pointer": function_pointer,
        "alias": false,
        "enum_member": false
    })
}

pub(super) fn symbol(
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

pub(super) fn declaration_ids(
    file: &super::super::super::model::SourceFile,
) -> HashMap<String, String> {
    file.declarations
        .iter()
        .map(|value| (value.qualified_name.clone(), value.id.clone()))
        .collect()
}

pub(super) fn edge(records: &[FactRecord], source: &str, target: &str, relation: &str) -> bool {
    records.iter().any(|record| {
        matches!(
            record,
            FactRecord::Edge(value)
                if value.source == source
                    && value.target == target
                    && value.relation == relation
        )
    })
}

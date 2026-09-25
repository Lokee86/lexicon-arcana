use super::{ADAPTER_VERSION, parser::ParsedFile};
use crate::{
    AdapterMode, AdapterRequest, Analysis, EdgeRecord, FACT_SCHEMA_VERSION, FactHeader, FactRecord,
    NodeRecord, UnresolvedRecord, content_id, node_id,
};
use serde_json::{Map, Value, json};
use std::path::Path;

pub fn analysis(root: &Path, request: &AdapterRequest, files: Vec<ParsedFile>) -> Analysis {
    let mut records = Vec::with_capacity(files.len() * 4);
    for file in files {
        add_file_records(&file, &mut records);
    }

    let incremental = request.mode == AdapterMode::Incremental;
    Analysis::new(
        FactHeader {
            adapter_version: ADAPTER_VERSION.into(),
            changed_files: incremental.then(|| normalized(&request.changed_files)),
            language: "c-family".into(),
            mode: incremental.then(|| "incremental".into()),
            record: "lexicon".into(),
            removed_files: incremental.then(|| normalized(&request.removed_files)),
            repository: root
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("repository")
                .into(),
            schema_version: FACT_SCHEMA_VERSION,
            shared_complete: incremental.then_some(false),
        },
        records,
    )
}

fn add_file_records(file: &ParsedFile, records: &mut Vec<FactRecord>) {
    let file_id = node_id("c-family", "file", &file.path);
    let module_id = node_id("c-family", "module", &file.path);
    let name = Path::new(&file.path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(&file.path);
    let stem = Path::new(&file.path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(name);

    let mut attributes = Map::new();
    attributes.insert("language".into(), Value::String(file.language.clone()));
    attributes.insert("parser".into(), Value::String("tree-sitter".into()));
    attributes.insert(
        "parser_language".into(),
        Value::String(file.parser_language.clone()),
    );
    if file.parse_error {
        attributes.insert("parse_error".into(), Value::Bool(true));
    }

    records.push(FactRecord::Node(NodeRecord {
        attributes: Some(Value::Object(attributes)),
        content_id: Some(content_id(&file.content)),
        id: file_id.clone(),
        kind: "file".into(),
        name: name.into(),
        owner: Some(file.path.clone()),
        path: file.path.clone(),
        qualified_name: file.path.clone(),
        span: None,
    }));
    records.push(FactRecord::Node(NodeRecord {
        attributes: Some(json!({ "language": file.language })),
        content_id: None,
        id: module_id.clone(),
        kind: "module".into(),
        name: stem.into(),
        owner: Some(file.path.clone()),
        path: file.path.clone(),
        qualified_name: file.path.clone(),
        span: None,
    }));
    records.push(FactRecord::Edge(EdgeRecord {
        attributes: None,
        owner: Some(file.path.clone()),
        relation: "contains".into(),
        source: file_id,
        span: None,
        target: module_id.clone(),
    }));

    if file.parse_error {
        records.push(FactRecord::Unresolved(UnresolvedRecord {
            attributes: Some(json!({ "parser": "tree-sitter" })),
            candidate_name: None,
            candidate_namespace: None,
            expression: file.path.clone(),
            owner: Some(file.path.clone()),
            reason: "unsupported-form".into(),
            relation: "references".into(),
            source: module_id,
            span: None,
        }));
    }
}

fn normalized(paths: &[String]) -> Vec<String> {
    let mut values = paths
        .iter()
        .map(|path| path.replace('\\', "/"))
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

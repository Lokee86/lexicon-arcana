use super::{
    ADAPTER_VERSION, include_facts,
    includes::FileIndex,
    model::{Declaration, RepositoryModel, SourceFile},
    visibility::VisibilityIndex,
};
use crate::{
    AdapterMode, AdapterRequest, Analysis, EdgeRecord, FACT_SCHEMA_VERSION, FactHeader, FactRecord,
    NodeRecord, UnresolvedRecord, content_id, node_id,
};
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, path::Path};

pub fn analysis(request: &AdapterRequest, model: RepositoryModel) -> Analysis {
    let mut records = Vec::new();
    let files = FileIndex::new(&model.files);
    for file in &model.files {
        add_file_records(file, &files, &model.visibility, &mut records);
    }
    super::relationship_facts::add(&model, &mut records);
    super::call_facts::add(&model, &mut records);
    super::dataflow::add_access_facts(&model, &mut records);
    records = deduplicate(records);

    let incremental = request.mode == AdapterMode::Incremental;
    Analysis::new(
        FactHeader {
            adapter_version: ADAPTER_VERSION.into(),
            changed_files: incremental.then(|| normalized(&request.changed_files)),
            language: "c-family".into(),
            mode: Some(if incremental { "incremental" } else { "full" }.into()),
            record: "lexicon".into(),
            removed_files: incremental.then(|| normalized(&request.removed_files)),
            repository: model.repository.clone(),
            schema_version: FACT_SCHEMA_VERSION,
            shared_complete: incremental.then_some(false),
        },
        records,
    )
}

fn add_file_records(
    file: &SourceFile,
    files: &FileIndex<'_>,
    visibility: &VisibilityIndex,
    records: &mut Vec<FactRecord>,
) {
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

    add_declarations(file, visibility, records);
    include_facts::add(file, files, visibility, records);

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

fn add_declarations(
    file: &SourceFile,
    visibility: &VisibilityIndex,
    records: &mut Vec<FactRecord>,
) {
    let mut nodes = BTreeMap::<String, &Declaration>::new();
    for declaration in &file.declarations {
        nodes.insert(declaration.id.clone(), declaration);
        debug_assert!(visibility.declaration_visible(&file.path, declaration));
        records.push(FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner: Some(declaration.path.clone()),
            relation: "defines".into(),
            source: declaration.container_id.clone(),
            span: Some(declaration.span.clone()),
            target: declaration.id.clone(),
        }));
    }
    for declaration in nodes.values() {
        records.push(FactRecord::Node(NodeRecord {
            attributes: Some(Value::Object(declaration.attributes.clone())),
            content_id: None,
            id: declaration.id.clone(),
            kind: declaration.kind.clone(),
            name: declaration.name.clone(),
            owner: Some(declaration.path.clone()),
            path: declaration.path.clone(),
            qualified_name: declaration.qualified_name.clone(),
            span: Some(declaration.span.clone()),
        }));
    }
}

fn deduplicate(records: Vec<FactRecord>) -> Vec<FactRecord> {
    let mut unique = BTreeMap::<String, FactRecord>::new();
    for record in records {
        let key = match &record {
            FactRecord::Node(node) => format!("node\0{}", node.id),
            FactRecord::Edge(edge) => format!(
                "edge\0{}\0{}\0{}\0{}",
                edge.source,
                edge.target,
                edge.relation,
                span_key(edge.span.as_ref())
            ),
            FactRecord::Unresolved(value) => format!(
                "unresolved\0{}\0{}\0{}\0{}\0{}",
                value.source,
                value.relation,
                value.expression,
                value.reason,
                span_key(value.span.as_ref())
            ),
        };
        unique.insert(key, record);
    }
    unique.into_values().collect()
}

fn span_key(span: Option<&crate::SourceSpan>) -> String {
    span.map(|span| {
        format!(
            "{}\0{:08}\0{:08}\0{:08}\0{:08}",
            span.path, span.start_line, span.start_column, span.end_line, span.end_column
        )
    })
    .unwrap_or_default()
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

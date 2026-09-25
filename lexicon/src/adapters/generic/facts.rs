use serde_json::json;

use crate::{
    EdgeRecord, FactRecord, NodeRecord, SourceSpan, UnresolvedRecord, content_id, node_id,
};

pub struct Facts {
    language: String,
    records: Vec<FactRecord>,
}

impl Facts {
    pub fn new(language: String) -> Self {
        Self {
            language,
            records: Vec::new(),
        }
    }

    pub fn add_file(&mut self, path: &str, content: &[u8]) -> String {
        let file_id = node_id(&self.language, "file", path);
        let module_id = node_id(&self.language, "module", path);
        let name = std::path::Path::new(path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(path);
        let stem = std::path::Path::new(path)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(name);

        self.records.push(FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: Some(content_id(content)),
            id: file_id.clone(),
            kind: "file".into(),
            name: name.into(),
            owner: Some(path.into()),
            path: path.into(),
            qualified_name: path.into(),
            span: None,
        }));
        self.records.push(FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: None,
            id: module_id.clone(),
            kind: "module".into(),
            name: stem.into(),
            owner: Some(path.into()),
            path: path.into(),
            qualified_name: path.into(),
            span: None,
        }));
        self.records.push(FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner: Some(path.into()),
            relation: "contains".into(),
            source: file_id,
            span: None,
            target: module_id.clone(),
        }));
        module_id
    }

    pub fn add_declaration(
        &mut self,
        path: &str,
        module_id: &str,
        kind: &str,
        name: &str,
        line_number: u64,
        line: &str,
    ) {
        let span = line_span(path, line_number, line);
        let canonical = format!("{path}::{kind}:{name}:{line_number}");
        let id = node_id(&self.language, kind, &canonical);

        self.records.push(FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: None,
            id: id.clone(),
            kind: kind.into(),
            name: name.into(),
            owner: Some(path.into()),
            path: path.into(),
            qualified_name: format!("{path}::{name}"),
            span: Some(span.clone()),
        }));
        self.records.push(FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner: Some(path.into()),
            relation: "defines".into(),
            source: module_id.into(),
            span: Some(span),
            target: id,
        }));
    }

    pub fn add_import(
        &mut self,
        path: &str,
        module_id: &str,
        keyword: &str,
        target: &str,
        line_number: u64,
        line: &str,
    ) {
        let span = line_span(path, line_number, line);
        let canonical = format!("{path}::import:{line_number}:{target}");
        let id = node_id(&self.language, "import", &canonical);

        self.records.push(FactRecord::Node(NodeRecord {
            attributes: Some(json!({
                "expression": line.trim(),
                "keyword": keyword,
                "target": target,
            })),
            content_id: None,
            id: id.clone(),
            kind: "import".into(),
            name: target.into(),
            owner: Some(path.into()),
            path: path.into(),
            qualified_name: canonical,
            span: Some(span.clone()),
        }));
        self.records.push(FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner: Some(path.into()),
            relation: "defines".into(),
            source: module_id.into(),
            span: Some(span.clone()),
            target: id.clone(),
        }));
        self.records.push(FactRecord::Unresolved(UnresolvedRecord {
            attributes: None,
            candidate_name: None,
            candidate_namespace: None,
            expression: target.into(),
            owner: Some(path.into()),
            reason: "external-target".into(),
            relation: "imports".into(),
            source: id,
            span: Some(span),
        }));
    }

    pub fn language_suffix(&self) -> &str {
        self.language
            .strip_prefix("generic-")
            .unwrap_or(&self.language)
    }

    pub fn into_records(self) -> Vec<FactRecord> {
        self.records
    }
}

fn line_span(path: &str, line_number: u64, line: &str) -> SourceSpan {
    SourceSpan {
        end_column: line.chars().count() as u64 + 1,
        end_line: line_number,
        path: path.into(),
        start_column: 1,
        start_line: line_number,
    }
}

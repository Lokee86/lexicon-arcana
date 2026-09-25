use super::{
    includes::FileIndex,
    model::{IncludeObservation, SourceFile},
    visibility::VisibilityIndex,
};
use crate::{EdgeRecord, FactRecord, NodeRecord, UnresolvedRecord, node_id};
use serde_json::json;

pub fn add(
    file: &SourceFile,
    files: &FileIndex<'_>,
    visibility: &VisibilityIndex,
    records: &mut Vec<FactRecord>,
) {
    for include in &file.includes {
        records.push(FactRecord::Node(NodeRecord {
            attributes: Some(json!({
                "expression": include.expression,
                "language": file.language,
                "system": include.system,
                "target": include.target,
            })),
            content_id: None,
            id: include.id.clone(),
            kind: "import".into(),
            name: include.target.clone(),
            owner: Some(file.path.clone()),
            path: file.path.clone(),
            qualified_name: format!("{}::include::{}", file.path, include.target),
            span: Some(include.span.clone()),
        }));
        records.push(FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner: Some(file.path.clone()),
            relation: "defines".into(),
            source: include.module_id.clone(),
            span: Some(include.span.clone()),
            target: include.id.clone(),
        }));
        resolve(include, files, visibility, records);
    }
}

fn resolve(
    include: &IncludeObservation,
    files: &FileIndex<'_>,
    visibility: &VisibilityIndex,
    records: &mut Vec<FactRecord>,
) {
    if let Some(target) = files.local_target(include) {
        debug_assert!(
            visibility
                .include_rank(&include.path, &target.path)
                .is_some()
        );
        records.push(FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner: Some(include.path.clone()),
            relation: "includes".into(),
            source: include.id.clone(),
            span: Some(include.span.clone()),
            target: node_id("c-family", "file", &target.path),
        }));
        return;
    }

    records.push(FactRecord::Unresolved(UnresolvedRecord {
        attributes: None,
        candidate_name: None,
        candidate_namespace: None,
        expression: include.expression.clone(),
        owner: Some(include.path.clone()),
        reason: if include.system {
            "external-target".into()
        } else {
            "missing-target".into()
        },
        relation: "imports".into(),
        source: include.id.clone(),
        span: Some(include.span.clone()),
    }));
}

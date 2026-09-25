use std::collections::BTreeMap;

use serde_json::Value;

use crate::{
    EdgeRecord, FactRecord, NodeRecord, SourceSpan, UnresolvedRecord, content_id, node_id,
};

#[derive(Default)]
pub struct Facts {
    nodes: BTreeMap<String, NodeRecord>,
    edges: BTreeMap<String, EdgeRecord>,
    unresolved: BTreeMap<String, UnresolvedRecord>,
}

impl Facts {
    #[allow(clippy::too_many_arguments)]
    pub fn add_node(
        &mut self,
        kind: &str,
        canonical: &str,
        name: &str,
        path: &str,
        qualified_name: &str,
        owner: Option<&str>,
        span: Option<SourceSpan>,
        attributes: Option<Value>,
    ) -> String {
        let id = node_id("kotlin", kind, canonical);
        let record = NodeRecord {
            attributes,
            content_id: None,
            id: id.clone(),
            kind: kind.into(),
            name: name.into(),
            owner: owner.map(str::to_owned),
            path: path.into(),
            qualified_name: qualified_name.into(),
            span,
        };
        match self.nodes.get(&id) {
            Some(existing) if existing.owner.is_some() || record.owner.is_none() => {}
            _ => {
                self.nodes.insert(id.clone(), record);
            }
        }
        id
    }

    pub fn add_file(&mut self, path: &str, content: &[u8], span: SourceSpan) -> String {
        let id = self.add_node(
            "file",
            path,
            std::path::Path::new(path)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(path),
            path,
            path,
            Some(path),
            Some(span),
            None,
        );
        if let Some(node) = self.nodes.get_mut(&id) {
            node.content_id = Some(content_id(content));
        }
        id
    }

    pub fn add_edge(
        &mut self,
        source: &str,
        target: &str,
        relation: &str,
        owner: Option<&str>,
        span: Option<SourceSpan>,
        attributes: Option<Value>,
    ) {
        let record = EdgeRecord {
            attributes,
            owner: owner.map(str::to_owned),
            relation: relation.into(),
            source: source.into(),
            span,
            target: target.into(),
        };
        let key = format!(
            "{}\0{}\0{}\0{:?}\0{}",
            record.source,
            record.target,
            record.relation,
            record.span,
            record
                .attributes
                .as_ref()
                .map_or(String::new(), Value::to_string)
        );
        self.edges.insert(key, record);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_unresolved(
        &mut self,
        source: &str,
        relation: &str,
        expression: &str,
        reason: &str,
        owner: Option<&str>,
        span: Option<SourceSpan>,
        attributes: Option<Value>,
    ) {
        let record = UnresolvedRecord {
            attributes,
            candidate_name: None,
            candidate_namespace: None,
            expression: expression.into(),
            owner: owner.map(str::to_owned),
            reason: reason.into(),
            relation: relation.into(),
            source: source.into(),
            span,
        };
        let key = format!(
            "{}\0{}\0{}\0{}\0{:?}\0{}",
            record.source,
            record.relation,
            record.expression,
            record.reason,
            record.span,
            record
                .attributes
                .as_ref()
                .map_or(String::new(), Value::to_string)
        );
        self.unresolved.insert(key, record);
    }

    pub fn node_attributes_mut(&mut self, id: &str) -> Option<&mut Option<Value>> {
        self.nodes.get_mut(id).map(|node| &mut node.attributes)
    }

    pub fn into_records(self) -> Vec<FactRecord> {
        self.nodes
            .into_values()
            .map(FactRecord::Node)
            .chain(self.edges.into_values().map(FactRecord::Edge))
            .chain(self.unresolved.into_values().map(FactRecord::Unresolved))
            .collect()
    }
}

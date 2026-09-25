use std::collections::BTreeMap;

use serde_json::Value;

use crate::{EdgeRecord, FactRecord, NodeRecord, SourceSpan, UnresolvedRecord, node_id};

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
        name: &str,
        path: &str,
        qualified_name: &str,
        identity: &str,
        owner: Option<&str>,
        span: Option<SourceSpan>,
        attributes: Option<Value>,
        content_id: Option<&str>,
    ) -> String {
        let id = node_id("lotusscript", kind, identity);
        let record = NodeRecord {
            attributes,
            content_id: content_id.map(str::to_owned),
            id: id.clone(),
            kind: kind.into(),
            name: name.into(),
            owner: owner.map(str::to_owned),
            path: path.into(),
            qualified_name: qualified_name.into(),
            span,
        };
        match self.nodes.get(&id) {
            Some(previous) if stable_json(previous) <= stable_json(&record) => {}
            _ => {
                self.nodes.insert(id.clone(), record);
            }
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
        self.edges.entry(edge_key(&record)).or_insert(record);
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
        self.unresolved
            .entry(unresolved_key(&record))
            .or_insert(record);
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

fn stable_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

fn edge_key(value: &EdgeRecord) -> String {
    format!(
        "{}\0{}\0{}\0{:?}\0{}",
        value.source,
        value.target,
        value.relation,
        value.span,
        value
            .attributes
            .as_ref()
            .map_or(String::new(), Value::to_string)
    )
}

fn unresolved_key(value: &UnresolvedRecord) -> String {
    format!(
        "{}\0{}\0{}\0{}\0{:?}\0{}",
        value.source,
        value.relation,
        value.expression,
        value.reason,
        value.span,
        value
            .attributes
            .as_ref()
            .map_or(String::new(), Value::to_string)
    )
}

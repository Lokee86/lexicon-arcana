use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::{
    EdgeRecord, FactRecord, NodeRecord, SourceSpan, UnresolvedRecord,
    content_id as lexicon_content_id, node_id,
};

pub(crate) struct Facts {
    pub(crate) nodes: BTreeMap<String, NodeRecord>,
    pub(crate) edges: BTreeMap<String, EdgeRecord>,
    pub(crate) unresolved: BTreeMap<String, UnresolvedRecord>,
    pub(crate) dataflow_edges: BTreeSet<String>,
}

impl Facts {
    pub(crate) fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            unresolved: BTreeMap::new(),
            dataflow_edges: BTreeSet::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add_node(
        &mut self,
        language: &str,
        kind: &str,
        canonical: &str,
        name: &str,
        path: &str,
        qualified_name: &str,
        content_id: Option<String>,
        span: Option<Value>,
        attributes: BTreeMap<String, Value>,
    ) -> String {
        let id = stable_id(language, kind, canonical);
        let record = NodeRecord {
            attributes: Some(Value::Object(attributes.into_iter().collect())),
            content_id,
            id: id.clone(),
            kind: kind.into(),
            name: name.into(),
            owner: None,
            path: path.into(),
            qualified_name: qualified_name.into(),
            span: span.and_then(source_span),
        };
        self.nodes.entry(id.clone()).or_insert(record);
        id
    }

    pub(crate) fn add_edge(
        &mut self,
        source: &str,
        target: &str,
        relation: &str,
        span: Option<Value>,
    ) {
        let record = EdgeRecord {
            attributes: None,
            owner: None,
            relation: relation.into(),
            source: source.into(),
            span: span.and_then(source_span),
            target: target.into(),
        };
        self.edges.entry(edge_key(&record)).or_insert(record);
    }

    pub(crate) fn add_edge_with_attributes(
        &mut self,
        source: &str,
        target: &str,
        relation: &str,
        span: Option<Value>,
        attributes: BTreeMap<String, Value>,
    ) {
        let record = EdgeRecord {
            attributes: Some(Value::Object(attributes.into_iter().collect())),
            owner: None,
            relation: relation.into(),
            source: source.into(),
            span: span.and_then(source_span),
            target: target.into(),
        };
        self.edges.entry(edge_key(&record)).or_insert(record);
    }

    pub(crate) fn add_unresolved(
        &mut self,
        source: &str,
        relation: &str,
        expression: &str,
        reason: &str,
        span: Option<Value>,
    ) {
        let record = UnresolvedRecord {
            attributes: None,
            candidate_name: None,
            candidate_namespace: None,
            expression: expression.into(),
            owner: None,
            reason: reason.into(),
            relation: relation.into(),
            source: source.into(),
            span: span.and_then(source_span),
        };
        self.unresolved
            .entry(unresolved_key(&record))
            .or_insert(record);
    }

    pub(crate) fn add_dataflow_edge(
        &mut self,
        source: &str,
        target: &str,
        relation: &str,
        span: Option<Value>,
    ) {
        let key = format!("{source}\0{target}\0{relation}");
        if !self.dataflow_edges.insert(key) {
            return;
        }
        self.add_edge(source, target, relation, span);
    }

    pub(crate) fn into_records(self) -> Vec<FactRecord> {
        self.nodes
            .into_values()
            .map(FactRecord::Node)
            .chain(self.edges.into_values().map(FactRecord::Edge))
            .chain(self.unresolved.into_values().map(FactRecord::Unresolved))
            .collect()
    }
}

pub(crate) fn stable_id(language: &str, kind: &str, canonical: &str) -> String {
    node_id(language, kind, canonical)
}

pub(crate) fn content_id(content: &[u8]) -> String {
    lexicon_content_id(content)
}

fn source_span(value: Value) -> Option<SourceSpan> {
    serde_json::from_value(value).ok()
}

fn edge_key(value: &EdgeRecord) -> String {
    format!(
        "{}\0{}\0{}\0{}\0{}",
        value.source,
        value.target,
        value.relation,
        span_key(value.span.as_ref()),
        value
            .attributes
            .as_ref()
            .map_or(String::new(), Value::to_string)
    )
}

fn unresolved_key(value: &UnresolvedRecord) -> String {
    format!(
        "{}\0{}\0{}\0{}\0{}",
        value.source,
        value.relation,
        value.expression,
        value.reason,
        span_key(value.span.as_ref())
    )
}

fn span_key(value: Option<&SourceSpan>) -> String {
    value
        .and_then(|value| serde_json::to_string(value).ok())
        .unwrap_or_default()
}

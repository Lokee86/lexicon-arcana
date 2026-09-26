use std::collections::{HashMap, HashSet};

use crate::{EdgeRecord, FactRecord, NodeRecord};

pub(super) type EdgeKey = (String, String, String, String);

pub(super) struct FactIndex {
    nodes: HashSet<String>,
    node_owners: HashMap<String, Option<String>>,
    edges: HashSet<EdgeKey>,
}

impl FactIndex {
    pub(super) fn from_records(records: &[FactRecord]) -> Self {
        let mut index = Self {
            nodes: HashSet::new(),
            node_owners: HashMap::new(),
            edges: HashSet::new(),
        };
        for record in records {
            let FactRecord::Node(node) = record else {
                continue;
            };
            index.nodes.insert(node.id.clone());
            index
                .node_owners
                .insert(node.id.clone(), node.owner.clone());
        }
        index
    }

    pub(super) fn contains_node(&self, id: &str) -> bool {
        self.nodes.contains(id)
    }

    pub(super) fn node_owner(&self, id: &str) -> Option<String> {
        self.node_owners.get(id).cloned().flatten()
    }

    pub(super) fn push_node(&mut self, records: &mut Vec<FactRecord>, node: NodeRecord) -> bool {
        if !self.nodes.insert(node.id.clone()) {
            return false;
        }
        self.node_owners.insert(node.id.clone(), node.owner.clone());
        records.push(FactRecord::Node(node));
        true
    }

    pub(super) fn push_edge(&mut self, records: &mut Vec<FactRecord>, edge: EdgeRecord) -> bool {
        let span_key = edge
            .span
            .as_ref()
            .map(|value| {
                format!(
                    "{}:{}:{}:{}:{}",
                    value.path,
                    value.start_line,
                    value.start_column,
                    value.end_line,
                    value.end_column
                )
            })
            .unwrap_or_default();
        let key = (
            edge.source.clone(),
            edge.target.clone(),
            edge.relation.clone(),
            span_key,
        );
        if !self.edges.insert(key) {
            return false;
        }
        records.push(FactRecord::Edge(edge));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_node_ownership_without_using_records_as_a_lookup_table() {
        let mut records = vec![FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: None,
            id: "node-1".into(),
            kind: "function".into(),
            name: "f".into(),
            owner: Some("main.go".into()),
            path: "main.go".into(),
            qualified_name: "main.go::f".into(),
            span: None,
        })];
        let mut index = FactIndex::from_records(&records);

        assert!(index.contains_node("node-1"));
        assert_eq!(index.node_owner("node-1").as_deref(), Some("main.go"));
        assert!(!index.push_node(
            &mut records,
            NodeRecord {
                attributes: None,
                content_id: None,
                id: "node-1".into(),
                kind: "function".into(),
                name: "duplicate".into(),
                owner: None,
                path: "other.go".into(),
                qualified_name: "other.go::duplicate".into(),
                span: None,
            },
        ));
        assert_eq!(records.len(), 1);
        assert_eq!(index.node_owner("node-1").as_deref(), Some("main.go"));
    }
}

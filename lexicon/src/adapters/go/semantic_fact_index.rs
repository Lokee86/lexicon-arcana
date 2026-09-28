use std::collections::{HashMap, HashSet};

use crate::{AdapterError, EdgeRecord, FactRecord, NodeRecord};

use super::{discovery::Module, identities, semantic_identity_policy};

pub(super) type EdgeKey = (String, String, String, String);

pub(super) struct FactIndex {
    modules: Vec<Module>,
    nodes: HashSet<String>,
    node_owners: HashMap<String, Option<String>>,
    edges: HashSet<EdgeKey>,
    identity_ids: HashMap<String, String>,
    identity_cache_hits: u64,
    identity_cache_misses: u64,
}

impl FactIndex {
    pub(super) fn from_records(records: &[FactRecord], modules: &[Module]) -> Self {
        let mut index = Self {
            modules: modules.to_vec(),
            nodes: HashSet::new(),
            node_owners: HashMap::new(),
            edges: HashSet::new(),
            identity_ids: HashMap::new(),
            identity_cache_hits: 0,
            identity_cache_misses: 0,
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

    pub(super) fn canonical_semantic_identity(
        &self,
        semantic_key: &str,
    ) -> Result<String, AdapterError> {
        semantic_identity_policy::canonical_identity(&self.modules, semantic_key)
    }

    pub(super) fn canonical_target_identity(
        &self,
        semantic_key: &str,
        namespace: Option<&str>,
    ) -> Result<String, AdapterError> {
        semantic_identity_policy::canonical_target_identity(&self.modules, semantic_key, namespace)
    }

    pub(super) fn semantic_node_id(&mut self, semantic_key: &str) -> Result<String, AdapterError> {
        let identity = self.canonical_semantic_identity(semantic_key)?;
        self.node_id(&identity)
    }

    pub(super) fn semantic_node_id_for_kind(
        &mut self,
        semantic_key: &str,
        expected_kind: &str,
    ) -> Result<String, AdapterError> {
        let identity = self.canonical_semantic_identity(semantic_key)?;
        self.node_id_for_kind(&identity, expected_kind)
    }

    pub(super) fn node_id(&mut self, identity: &str) -> Result<String, AdapterError> {
        if let Some(id) = self.identity_ids.get(identity) {
            self.identity_cache_hits += 1;
            return Ok(id.clone());
        }
        let id = identities::node_id(identity)?;
        self.identity_ids.insert(identity.to_owned(), id.clone());
        self.identity_cache_misses += 1;
        Ok(id)
    }

    pub(super) fn node_id_for_kind(
        &mut self,
        identity: &str,
        expected_kind: &str,
    ) -> Result<String, AdapterError> {
        let actual = identities::lexicon_kind(identity)?;
        if actual != expected_kind {
            return Err(AdapterError::new(format!(
                "Go identity {identity:?} maps to {actual:?}, expected {expected_kind:?}"
            )));
        }
        self.node_id(identity)
    }

    pub(super) fn identity_cache_stats(&self) -> (u64, u64) {
        (self.identity_cache_hits, self.identity_cache_misses)
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
        let mut index = FactIndex::from_records(&records, &[]);

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

    #[test]
    fn memoizes_identity_to_node_id_within_materialization() {
        let mut index = FactIndex::from_records(&[], &[]);
        let identity = "function:example.com/app:run";

        let first = index.node_id(identity).unwrap();
        let second = index.node_id(identity).unwrap();

        assert_eq!(first, second);
        assert_eq!(index.identity_cache_stats(), (1, 1));
    }

    #[test]
    fn semantic_keys_are_canonicalized_before_hashing() {
        let modules = vec![Module {
            root: ".".into(),
            path: "example.com/app".into(),
        }];
        let mut index = FactIndex::from_records(&[], &modules);

        let from_frontend = index
            .semantic_node_id("function:example.com/app_test:run")
            .unwrap();
        let canonical = index.node_id("function:example.com/app:run").unwrap();

        assert_eq!(from_frontend, canonical);
    }
}

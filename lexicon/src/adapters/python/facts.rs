use std::collections::BTreeMap;

use serde_json::Value;

use crate::{
    EdgeRecord, FactRecord, NodeRecord, SourceSpan, UnresolvedRecord, content_id, node_id,
};

use super::model::{
    CallInfo, ClassInfo, FunctionInfo, ImportInfo, InheritanceInfo, LocalAssignmentInfo,
    LoopBindingInfo,
};

pub struct Facts {
    pub repository: String,
    pub nodes: BTreeMap<String, NodeRecord>,
    pub edges: BTreeMap<String, EdgeRecord>,
    pub unresolved: BTreeMap<String, UnresolvedRecord>,
    pub modules: BTreeMap<String, String>,
    pub symbols: BTreeMap<String, String>,
    pub qnames: BTreeMap<String, String>,
    pub imports: Vec<ImportInfo>,
    pub inheritances: Vec<InheritanceInfo>,
    pub functions: BTreeMap<String, FunctionInfo>,
    pub classes: BTreeMap<String, ClassInfo>,
    pub lambda_ids: BTreeMap<(String, u32), String>,
    pub calls: Vec<CallInfo>,
    pub local_assignments: Vec<LocalAssignmentInfo>,
    pub loop_bindings: Vec<LoopBindingInfo>,
    pub module_bindings: BTreeMap<(String, String), (Option<String>, String)>,
    pub scope_bindings: BTreeMap<(String, String), (Option<String>, String)>,
    pub scope_parents: BTreeMap<String, String>,
    pub data_symbols: BTreeMap<(String, String), String>,
}

impl Facts {
    pub fn new(repository: String) -> Self {
        Self {
            repository,
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            unresolved: BTreeMap::new(),
            modules: BTreeMap::new(),
            symbols: BTreeMap::new(),
            qnames: BTreeMap::new(),
            imports: Vec::new(),
            inheritances: Vec::new(),
            functions: BTreeMap::new(),
            classes: BTreeMap::new(),
            lambda_ids: BTreeMap::new(),
            calls: Vec::new(),
            local_assignments: Vec::new(),
            loop_bindings: Vec::new(),
            module_bindings: BTreeMap::new(),
            scope_bindings: BTreeMap::new(),
            scope_parents: BTreeMap::new(),
            data_symbols: BTreeMap::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_node(
        &mut self,
        kind: &str,
        name: &str,
        path: &str,
        qname: &str,
        identity: Option<&str>,
        span: Option<SourceSpan>,
        attributes: Option<Value>,
        content: Option<&[u8]>,
    ) -> String {
        let id = node_id("python", kind, identity.unwrap_or(qname));
        self.nodes.insert(
            id.clone(),
            NodeRecord {
                attributes,
                content_id: content.map(content_id),
                id: id.clone(),
                kind: kind.into(),
                name: name.into(),
                owner: None,
                path: path.into(),
                qualified_name: qname.into(),
                span,
            },
        );
        self.qnames.insert(id.clone(), qname.into());
        id
    }

    pub fn add_edge(
        &mut self,
        source: &str,
        target: &str,
        relation: &str,
        span: Option<SourceSpan>,
        attributes: Option<Value>,
    ) {
        let edge = EdgeRecord {
            attributes,
            owner: None,
            relation: relation.into(),
            source: source.into(),
            span,
            target: target.into(),
        };
        self.edges.entry(edge_key(&edge)).or_insert(edge);
    }

    pub fn add_unresolved(
        &mut self,
        source: &str,
        relation: &str,
        expression: &str,
        reason: &str,
        span: Option<SourceSpan>,
        candidate: Option<String>,
    ) {
        let value = UnresolvedRecord {
            attributes: None,
            candidate_name: candidate,
            candidate_namespace: None,
            expression: expression.into(),
            owner: None,
            reason: reason.into(),
            relation: relation.into(),
            source: source.into(),
            span,
        };
        self.unresolved
            .entry(unresolved_key(&value))
            .or_insert(value);
    }

    pub fn merge_from(&mut self, source: Facts) {
        for (key, value) in source.nodes {
            self.nodes.insert(key, value);
        }
        for (key, value) in source.edges {
            self.edges.insert(key, value);
        }
        for (key, value) in source.unresolved {
            self.unresolved.insert(key, value);
        }
        for (key, value) in source.modules {
            self.modules.insert(key, value);
        }
        for (key, value) in source.symbols {
            self.symbols.insert(key, value);
        }
        for (key, value) in source.qnames {
            self.qnames.insert(key, value);
        }
        self.imports.extend(source.imports);
        self.inheritances.extend(source.inheritances);
        for (key, value) in source.functions {
            self.functions.insert(key, value);
        }
        for (key, value) in source.classes {
            self.classes.insert(key, value);
        }
        for (key, value) in source.lambda_ids {
            self.lambda_ids.insert(key, value);
        }
        self.calls.extend(source.calls);
        self.local_assignments.extend(source.local_assignments);
        self.loop_bindings.extend(source.loop_bindings);
        for (key, value) in source.module_bindings {
            self.module_bindings.insert(key, value);
        }
        for (key, value) in source.scope_bindings {
            self.scope_bindings.insert(key, value);
        }
        for (key, value) in source.scope_parents {
            self.scope_parents.insert(key, value);
        }
        for (key, value) in source.data_symbols {
            self.data_symbols.insert(key, value);
        }
    }

    pub fn release_analysis_state(&mut self) {
        self.modules.clear();
        self.symbols.clear();
        self.qnames.clear();
        self.imports.clear();
        self.inheritances.clear();
        self.functions.clear();
        self.classes.clear();
        self.lambda_ids.clear();
        self.calls.clear();
        self.local_assignments.clear();
        self.loop_bindings.clear();
        self.module_bindings.clear();
        self.scope_bindings.clear();
        self.scope_parents.clear();
        self.data_symbols.clear();
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
        value.candidate_name.as_deref().unwrap_or_default()
    )
}

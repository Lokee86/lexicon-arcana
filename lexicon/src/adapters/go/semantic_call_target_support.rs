use std::collections::BTreeSet;

use crate::{AdapterError, FactRecord, NodeRecord};

use super::{
    discovery::Inventory,
    identities,
    semantic_facts_support::{EdgeKey, push_edge},
};

pub(super) fn ensure_namespace(
    namespace: &str,
    path: &str,
    inventory: &Inventory,
    records: &mut Vec<FactRecord>,
    nodes: &mut BTreeSet<String>,
    edges: &mut BTreeSet<EdgeKey>,
) -> Result<String, AdapterError> {
    let id = identities::node_id(&identities::namespace(namespace))?;
    if nodes.insert(id.clone()) {
        records.push(FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: None,
            id: id.clone(),
            kind: "namespace".into(),
            name: namespace.into(),
            owner: None,
            path: path.into(),
            qualified_name: path.into(),
            span: None,
        }));
        push_edge(
            records,
            edges,
            identities::node_id(&identities::repository(&inventory.repository))?,
            id.clone(),
            "contains",
            None,
            None,
        );
    }
    Ok(id)
}

pub(super) struct SyntheticNode<'a> {
    pub id: String,
    pub kind: &'a str,
    pub name: &'a str,
    pub path: &'a str,
    pub namespace: &'a str,
}

pub(super) fn ensure_node(
    node: SyntheticNode<'_>,
    records: &mut Vec<FactRecord>,
    nodes: &mut BTreeSet<String>,
    edges: &mut BTreeSet<EdgeKey>,
) -> Result<(), AdapterError> {
    let namespace_id = identities::node_id(&identities::namespace(node.namespace))?;
    if nodes.insert(node.id.clone()) {
        records.push(FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: None,
            id: node.id.clone(),
            kind: node.kind.into(),
            name: node.name.into(),
            owner: None,
            path: node.path.into(),
            qualified_name: format!("{}::{}", node.path, node.name),
            span: None,
        }));
        push_edge(records, edges, namespace_id, node.id, "defines", None, None);
    }
    Ok(())
}

pub(super) fn callable_identity(identity: &str) -> Result<(&str, &str), AdapterError> {
    let body = identity
        .strip_prefix("function:")
        .or_else(|| identity.strip_prefix("method:"))
        .ok_or_else(|| AdapterError::new(format!("invalid Go callable identity {identity:?}")))?;
    let (namespace, tail) = body
        .rsplit_once(':')
        .ok_or_else(|| AdapterError::new(format!("invalid Go callable identity {identity:?}")))?;
    let name = tail.rsplit_once('.').map(|(_, name)| name).unwrap_or(tail);
    if namespace.is_empty() || name.is_empty() {
        return Err(AdapterError::new(format!(
            "invalid Go callable identity {identity:?}"
        )));
    }
    Ok((namespace, name))
}

pub(super) fn named_type_identity(identity: &str) -> Result<(&str, &str), AdapterError> {
    let body = identity
        .strip_prefix("type:")
        .ok_or_else(|| AdapterError::new(format!("invalid Go type identity {identity:?}")))?;
    body.rsplit_once(':')
        .ok_or_else(|| AdapterError::new(format!("invalid Go type identity {identity:?}")))
}

pub(super) fn namespace_path(namespace: &str) -> String {
    if namespace == "go:unknown" {
        "@external/go-unknown".into()
    } else if is_standard_library_namespace(namespace) {
        format!("@stdlib/{namespace}")
    } else {
        format!("@external/{namespace}")
    }
}

fn is_standard_library_namespace(namespace: &str) -> bool {
    let first = namespace.split('/').next().unwrap_or(namespace);
    !namespace.is_empty() && !namespace.contains(':') && !first.contains('.')
}

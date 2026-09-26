use std::collections::{BTreeMap, BTreeSet};

use crate::{AdapterError, EdgeRecord, FactRecord, NodeRecord, SourceSpan};

use super::{discovery::Inventory, identities};

pub(super) type EdgeKey = (String, String, String, String);

pub(super) fn container_id(metadata: &BTreeMap<String, String>) -> Result<String, AdapterError> {
    identities::node_id(required(metadata, "container")?)
}

pub(super) fn required<'a>(
    metadata: &'a BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str, AdapterError> {
    metadata
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AdapterError::new(format!("Go semantic declaration is missing {key}")))
}

pub(super) fn parent_id(owner: &str, inventory: &Inventory) -> Result<String, AdapterError> {
    match owner.rsplit_once('/') {
        Some((parent, _)) => identities::node_id(&identities::directory(parent)),
        None => identities::node_id(&identities::repository(&inventory.repository)),
    }
}

pub(super) fn ensure_relationship_target(
    identity: &str,
    inventory: &Inventory,
    records: &mut Vec<FactRecord>,
    nodes: &mut BTreeSet<String>,
    edges: &mut BTreeSet<EdgeKey>,
) -> Result<String, AdapterError> {
    let id = identities::node_id(identity)?;
    if nodes.contains(&id) {
        return Ok(id);
    }
    let (namespace, name) = external_named_type(identity, inventory)?;
    let path = namespace_path(namespace);
    let namespace_id = identities::node_id(&identities::namespace(namespace))?;

    if nodes.insert(namespace_id.clone()) {
        records.push(FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: None,
            id: namespace_id.clone(),
            kind: "namespace".into(),
            name: namespace.into(),
            owner: None,
            path: path.clone(),
            qualified_name: path.clone(),
            span: None,
        }));
        push_edge(
            records,
            edges,
            identities::node_id(&identities::repository(&inventory.repository))?,
            namespace_id.clone(),
            "contains",
            None,
            None,
        );
    }
    if nodes.insert(id.clone()) {
        records.push(FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: None,
            id: id.clone(),
            kind: "type".into(),
            name: name.into(),
            owner: None,
            path: path.clone(),
            qualified_name: format!("{path}::{name}"),
            span: None,
        }));
        push_edge(
            records,
            edges,
            namespace_id,
            id.clone(),
            "defines",
            None,
            None,
        );
    }
    Ok(id)
}

fn external_named_type<'a>(
    identity: &'a str,
    inventory: &Inventory,
) -> Result<(&'a str, &'a str), AdapterError> {
    let body = identity.strip_prefix("type:").ok_or_else(|| {
        AdapterError::new(format!(
            "Go semantic relationship target is not materialized: {identity:?}"
        ))
    })?;
    let (namespace, name) = body.rsplit_once(':').ok_or_else(|| {
        AdapterError::new(format!("invalid Go type relationship target {identity:?}"))
    })?;
    let internal = inventory.modules.iter().any(|module| {
        namespace == module.path || namespace.starts_with(&format!("{}/", module.path))
    });
    if internal || namespace.is_empty() || name.is_empty() {
        return Err(AdapterError::new(format!(
            "Go semantic relationship target is not materialized: {identity:?}"
        )));
    }
    Ok((namespace, name))
}

fn namespace_path(namespace: &str) -> String {
    if is_standard_library_namespace(namespace) {
        format!("@stdlib/{namespace}")
    } else {
        format!("@external/{namespace}")
    }
}

fn is_standard_library_namespace(namespace: &str) -> bool {
    if namespace.is_empty() || namespace.contains(':') {
        return false;
    }
    let first = namespace.split('/').next().unwrap_or(namespace);
    !first.contains('.')
}

pub(super) fn node_owner(records: &[FactRecord], id: &str) -> Option<String> {
    records.iter().find_map(|record| match record {
        FactRecord::Node(node) if node.id == id => node.owner.clone(),
        _ => None,
    })
}

pub(super) fn push_edge(
    records: &mut Vec<FactRecord>,
    seen: &mut BTreeSet<EdgeKey>,
    source: String,
    target: String,
    relation: &str,
    owner: Option<String>,
    span: Option<SourceSpan>,
) {
    let span_key = span
        .as_ref()
        .map(|value| {
            format!(
                "{}:{}:{}:{}:{}",
                value.path, value.start_line, value.start_column, value.end_line, value.end_column
            )
        })
        .unwrap_or_default();
    let key = (
        source.clone(),
        target.clone(),
        relation.to_owned(),
        span_key,
    );
    if seen.insert(key) {
        records.push(FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner,
            relation: relation.into(),
            source,
            span,
            target,
        }));
    }
}

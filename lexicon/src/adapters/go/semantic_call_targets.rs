use std::collections::BTreeSet;

use crate::{AdapterError, FactRecord};

use super::{
    discovery::Inventory,
    identities,
    protocol_records::CallClass,
    semantic_call_target_support::{
        SyntheticNode, callable_identity, ensure_namespace, ensure_node, named_type_identity,
        namespace_path,
    },
    semantic_facts_support::EdgeKey,
};

pub(super) fn ensure_call_target(
    identity: &str,
    class: CallClass,
    inventory: &Inventory,
    records: &mut Vec<FactRecord>,
    nodes: &mut BTreeSet<String>,
    edges: &mut BTreeSet<EdgeKey>,
) -> Result<String, AdapterError> {
    let id = identities::node_id(identity)?;
    if nodes.contains(&id) {
        return Ok(id);
    }

    match class {
        CallClass::Internal | CallClass::Interface | CallClass::Dynamic => {
            return Err(AdapterError::new(format!(
                "Go semantic call target is not materialized: {identity:?}"
            )));
        }
        CallClass::Builtin => {
            let (namespace, name) = callable_identity(identity)?;
            ensure_namespace(
                "go:builtins",
                "@builtin/go",
                inventory,
                records,
                nodes,
                edges,
            )?;
            ensure_callable(
                identity,
                namespace,
                name,
                "@builtin/go",
                records,
                nodes,
                edges,
            )?;
        }
        CallClass::External => {
            let (namespace, name) = callable_identity(identity)?;
            let path = namespace_path(namespace);
            ensure_namespace(namespace, &path, inventory, records, nodes, edges)?;
            ensure_callable(identity, namespace, name, &path, records, nodes, edges)?;
        }
        CallClass::Conversion => {
            ensure_type_target(identity, inventory, records, nodes, edges)?;
        }
    }
    Ok(id)
}

fn ensure_type_target(
    identity: &str,
    inventory: &Inventory,
    records: &mut Vec<FactRecord>,
    nodes: &mut BTreeSet<String>,
    edges: &mut BTreeSet<EdgeKey>,
) -> Result<(), AdapterError> {
    let id = identities::node_id(identity)?;
    if let Some(name) = identity.strip_prefix("type-expression:") {
        ensure_namespace("go:types", "@types/go", inventory, records, nodes, edges)?;
        ensure_node(
            SyntheticNode {
                id,
                kind: "type",
                name,
                path: "@types/go",
                namespace: "go:types",
            },
            records,
            nodes,
            edges,
        )?;
        return Ok(());
    }

    let (namespace, name) = named_type_identity(identity)?;
    if inventory.modules.iter().any(|module| {
        namespace == module.path || namespace.starts_with(&format!("{}/", module.path))
    }) {
        return Err(AdapterError::new(format!(
            "Go semantic conversion target is not materialized: {identity:?}"
        )));
    }
    let path = namespace_path(namespace);
    ensure_namespace(namespace, &path, inventory, records, nodes, edges)?;
    ensure_node(
        SyntheticNode {
            id,
            kind: "type",
            name,
            path: &path,
            namespace,
        },
        records,
        nodes,
        edges,
    )
}

fn ensure_callable(
    identity: &str,
    namespace: &str,
    name: &str,
    path: &str,
    records: &mut Vec<FactRecord>,
    nodes: &mut BTreeSet<String>,
    edges: &mut BTreeSet<EdgeKey>,
) -> Result<(), AdapterError> {
    let id = identities::node_id(identity)?;
    ensure_node(
        SyntheticNode {
            id,
            kind: identities::lexicon_kind(identity)?,
            name,
            path,
            namespace,
        },
        records,
        nodes,
        edges,
    )
}

use crate::{AdapterError, FactRecord};

use super::{
    discovery::Inventory,
    identities,
    semantic_call_target_support::{
        SyntheticNode, ensure_namespace, ensure_node, named_type_identity, namespace_path,
    },
    semantic_fact_index::FactIndex,
};

pub(super) fn ensure_type_target(
    identity: &str,
    inventory: &Inventory,
    records: &mut Vec<FactRecord>,
    index: &mut FactIndex,
) -> Result<(), AdapterError> {
    let id = index.node_id(identity)?;
    if let Some(name) = identity.strip_prefix("type-expression:") {
        ensure_namespace("go:types", "@types/go", inventory, records, index)?;
        ensure_node(
            SyntheticNode {
                id,
                kind: "type",
                name,
                path: "@types/go",
                namespace: "go:types",
            },
            records,
            index,
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
    ensure_namespace(namespace, &path, inventory, records, index)?;
    ensure_node(
        SyntheticNode {
            id,
            kind: "type",
            name,
            path: &path,
            namespace,
        },
        records,
        index,
    )
}

pub(super) fn ensure_callable(
    identity: &str,
    namespace: &str,
    name: &str,
    path: &str,
    records: &mut Vec<FactRecord>,
    index: &mut FactIndex,
) -> Result<(), AdapterError> {
    let id = index.node_id(identity)?;
    ensure_node(
        SyntheticNode {
            id,
            kind: identities::lexicon_kind(identity)?,
            name,
            path,
            namespace,
        },
        records,
        index,
    )
}

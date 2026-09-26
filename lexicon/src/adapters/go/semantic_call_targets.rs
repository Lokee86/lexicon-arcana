use std::collections::BTreeSet;

use crate::{AdapterError, FactRecord};

use super::{
    discovery::Inventory,
    identities,
    protocol_records::CallClass,
    semantic_call_contract_targets::{ensure_callable, ensure_type_target},
    semantic_call_target_support::{callable_identity, ensure_namespace, namespace_path},
    semantic_facts_support::EdgeKey,
};

pub(super) struct TargetHints<'a> {
    pub name: Option<&'a str>,
    pub namespace: Option<&'a str>,
    pub container: Option<&'a str>,
}

pub(super) struct TargetMaterialization<'a> {
    pub inventory: &'a Inventory,
    pub records: &'a mut Vec<FactRecord>,
    pub nodes: &'a mut BTreeSet<String>,
    pub edges: &'a mut BTreeSet<EdgeKey>,
}

pub(super) fn ensure_call_target(
    identity: &str,
    class: CallClass,
    hints: TargetHints<'_>,
    materialization: &mut TargetMaterialization<'_>,
) -> Result<String, AdapterError> {
    let id = identities::node_id(identity)?;
    if materialization.nodes.contains(&id) {
        return Ok(id);
    }

    match class {
        CallClass::Internal | CallClass::Interface => {
            return Err(AdapterError::new(format!(
                "Go semantic call target is not materialized: {identity:?}"
            )));
        }
        CallClass::Dynamic => {
            ensure_dynamic_target(identity, hints, materialization)?;
        }
        CallClass::Builtin => {
            let (namespace, name) = callable_identity(identity)?;
            ensure_namespace(
                "go:builtins",
                "@builtin/go",
                materialization.inventory,
                materialization.records,
                materialization.nodes,
                materialization.edges,
            )?;
            ensure_callable(
                identity,
                namespace,
                name,
                "@builtin/go",
                materialization.records,
                materialization.nodes,
                materialization.edges,
            )?;
        }
        CallClass::External => {
            let (namespace, name) = callable_identity(identity)?;
            let path = namespace_path(namespace);
            ensure_namespace(
                namespace,
                &path,
                materialization.inventory,
                materialization.records,
                materialization.nodes,
                materialization.edges,
            )?;
            ensure_callable(
                identity,
                namespace,
                name,
                &path,
                materialization.records,
                materialization.nodes,
                materialization.edges,
            )?;
        }
        CallClass::Conversion => {
            ensure_type_target(
                identity,
                materialization.inventory,
                materialization.records,
                materialization.nodes,
                materialization.edges,
            )?;
        }
    }
    Ok(id)
}

fn ensure_dynamic_target(
    identity: &str,
    hints: TargetHints<'_>,
    materialization: &mut TargetMaterialization<'_>,
) -> Result<(), AdapterError> {
    if identity.starts_with("ssa-function:") {
        return super::semantic_ssa_target_support::ensure_synthetic_function(
            identity,
            hints.name.unwrap_or(identity),
            hints.namespace.unwrap_or("go:ssa"),
            hints.container,
            materialization,
        );
    }
    if identity.starts_with("closure:") {
        return Err(AdapterError::new(format!(
            "Go semantic closure target is not materialized: {identity:?}"
        )));
    }
    let (namespace, name) = callable_identity(identity)?;
    let internal = materialization.inventory.modules.iter().any(|module| {
        namespace == module.path || namespace.starts_with(&format!("{}/", module.path))
    });
    if internal {
        return Err(AdapterError::new(format!(
            "Go semantic dynamic target is not materialized: {identity:?}"
        )));
    }
    let path = namespace_path(namespace);
    ensure_namespace(
        namespace,
        &path,
        materialization.inventory,
        materialization.records,
        materialization.nodes,
        materialization.edges,
    )?;
    ensure_callable(
        identity,
        namespace,
        name,
        &path,
        materialization.records,
        materialization.nodes,
        materialization.edges,
    )
}

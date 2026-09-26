use crate::{AdapterError, FactRecord, NodeRecord, SourceSpan};

use super::{
    discovery::Inventory,
    identities,
    protocol_records::CallClass,
    semantic_call_contract_targets::{ensure_callable, ensure_type_target},
    semantic_call_target_support::{callable_identity, ensure_namespace, namespace_path},
    semantic_fact_index::FactIndex,
};

pub(super) struct TargetHints<'a> {
    pub name: Option<&'a str>,
    pub namespace: Option<&'a str>,
    pub container: Option<&'a str>,
    pub owner: Option<&'a str>,
    pub span: Option<&'a super::protocol_records::Span>,
}

pub(super) struct TargetMaterialization<'a> {
    pub inventory: &'a Inventory,
    pub records: &'a mut Vec<FactRecord>,
    pub index: &'a mut FactIndex,
}

pub(super) fn ensure_call_target(
    identity: &str,
    class: CallClass,
    hints: TargetHints<'_>,
    materialization: &mut TargetMaterialization<'_>,
) -> Result<String, AdapterError> {
    let id = identities::node_id(identity)?;
    if materialization.index.contains_node(&id) {
        return Ok(id);
    }

    match class {
        CallClass::Internal => {
            if let (Some(name), Some(namespace)) = (hints.name, hints.namespace) {
                super::semantic_ssa_target_support::ensure_generated_internal_function(
                    identity,
                    name,
                    namespace,
                    hints.container,
                    materialization,
                )?;
            } else {
                return Err(AdapterError::new(format!(
                    "Go semantic call target is not materialized: {identity:?}"
                )));
            }
        }
        CallClass::Interface => {
            ensure_interface_target(identity, &hints, materialization)?;
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
                materialization.index,
            )?;
            ensure_callable(
                identity,
                namespace,
                name,
                "@builtin/go",
                materialization.records,
                materialization.index,
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
                materialization.index,
            )?;
            ensure_callable(
                identity,
                namespace,
                name,
                &path,
                materialization.records,
                materialization.index,
            )?;
        }
        CallClass::Conversion => {
            ensure_type_target(
                identity,
                materialization.inventory,
                materialization.records,
                materialization.index,
            )?;
        }
    }
    Ok(id)
}

fn ensure_interface_target(
    identity: &str,
    hints: &TargetHints<'_>,
    materialization: &mut TargetMaterialization<'_>,
) -> Result<(), AdapterError> {
    let (namespace, parsed_name) = callable_identity(identity)?;
    let internal = materialization.inventory.modules.iter().any(|module| {
        namespace == module.path || namespace.starts_with(&format!("{}/", module.path))
    });
    if internal {
        let name = hints.name.unwrap_or(parsed_name);
        let owner = hints.owner.ok_or_else(|| {
            AdapterError::new(format!(
                "Go semantic internal interface target is missing owner: {identity:?}"
            ))
        })?;
        let span = hints.span.ok_or_else(|| {
            AdapterError::new(format!(
                "Go semantic internal interface target is missing span: {identity:?}"
            ))
        })?;
        let container = hints.container.ok_or_else(|| {
            AdapterError::new(format!(
                "Go semantic internal interface target is missing container: {identity:?}"
            ))
        })?;
        let parent = identities::node_id(container)?;
        if !materialization.index.contains_node(&parent) {
            return Err(AdapterError::new(format!(
                "Go semantic internal interface container is not materialized: {container:?}"
            )));
        }
        let id = identities::node_id(identity)?;
        let location = SourceSpan {
            path: owner.into(),
            start_line: span.start_line,
            start_column: span.start_column,
            end_line: span.end_line,
            end_column: span.end_column,
        };
        if materialization.index.push_node(
            materialization.records,
            NodeRecord {
                attributes: None,
                content_id: None,
                id: id.clone(),
                kind: identities::lexicon_kind(identity)?.into(),
                name: name.into(),
                owner: Some(owner.into()),
                path: owner.into(),
                qualified_name: format!("{owner}::{name}"),
                span: Some(location.clone()),
            },
        ) {
            super::semantic_facts_support::push_edge(
                materialization.records,
                materialization.index,
                parent,
                id,
                "defines",
                Some(owner.into()),
                Some(location),
            );
        }
        return Ok(());
    }
    let path = namespace_path(namespace);
    ensure_namespace(
        namespace,
        &path,
        materialization.inventory,
        materialization.records,
        materialization.index,
    )?;
    ensure_callable(
        identity,
        namespace,
        parsed_name,
        &path,
        materialization.records,
        materialization.index,
    )
}

fn ensure_dynamic_target(
    identity: &str,
    hints: TargetHints<'_>,
    materialization: &mut TargetMaterialization<'_>,
) -> Result<(), AdapterError> {
    if let Some(body) = identity.strip_prefix("dynamic-method:") {
        let (_, name) = body.rsplit_once('.').ok_or_else(|| {
            AdapterError::new(format!("invalid Go dynamic method identity {identity:?}"))
        })?;
        ensure_namespace(
            "go:types",
            "@types/go",
            materialization.inventory,
            materialization.records,
            materialization.index,
        )?;
        return super::semantic_call_target_support::ensure_node(
            super::semantic_call_target_support::SyntheticNode {
                id: identities::node_id(identity)?,
                kind: "method",
                name,
                path: "@types/go",
                namespace: "go:types",
            },
            materialization.records,
            materialization.index,
        );
    }
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
        materialization.index,
    )?;
    ensure_callable(
        identity,
        namespace,
        name,
        &path,
        materialization.records,
        materialization.index,
    )
}

use crate::{AdapterError, FactRecord, NodeRecord, SourceSpan};

use super::{
    discovery::Inventory,
    identities,
    observations::{CallForm, Span, SymbolReference},
    semantic_call_contract_targets::{ensure_callable, ensure_type_target},
    semantic_call_target_support::{callable_identity, ensure_namespace, namespace_path},
    semantic_fact_index::FactIndex,
};

pub(super) struct TargetEvidence<'a> {
    pub semantic_key: &'a str,
    pub name: Option<&'a str>,
    pub namespace: Option<&'a str>,
    pub container_key: Option<&'a str>,
    pub owner: Option<&'a str>,
    pub span: Option<&'a Span>,
    pub generated: bool,
}

impl<'a> From<&'a SymbolReference> for TargetEvidence<'a> {
    fn from(value: &'a SymbolReference) -> Self {
        Self {
            semantic_key: &value.semantic_key,
            name: value.name.as_deref(),
            namespace: value.namespace.as_deref(),
            container_key: value.container_key.as_deref(),
            owner: value.owner.as_deref(),
            span: value.span.as_ref(),
            generated: value.generated,
        }
    }
}

pub(super) struct TargetMaterialization<'a> {
    pub inventory: &'a Inventory,
    pub records: &'a mut Vec<FactRecord>,
    pub index: &'a mut FactIndex,
}

pub(super) fn ensure_call_target(
    target: TargetEvidence<'_>,
    form: CallForm,
    materialization: &mut TargetMaterialization<'_>,
) -> Result<String, AdapterError> {
    let identity = target.semantic_key;
    let id = materialization.index.node_id(identity)?;
    if materialization.index.contains_node(&id) {
        return Ok(id);
    }

    if matches!(form, CallForm::Conversion)
        || identity.starts_with("type:")
        || identity.starts_with("type-expression:")
    {
        ensure_type_target(
            identity,
            materialization.inventory,
            materialization.records,
            materialization.index,
        )?;
        return Ok(id);
    }

    if matches!(form, CallForm::Builtin) || identity.starts_with("function:go:builtins:") {
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
        return Ok(id);
    }

    if matches!(form, CallForm::Interface) {
        ensure_interface_target(identity, &target, materialization)?;
        return Ok(id);
    }

    if identity.starts_with("dynamic-method:") || identity.starts_with("ssa-function:") {
        ensure_dynamic_target(identity, target, materialization)?;
        return Ok(id);
    }

    if identity.starts_with("closure:") {
        return Err(AdapterError::new(format!(
            "Go semantic closure target is not materialized: {identity:?}"
        )));
    }

    let (parsed_namespace, parsed_name) = callable_identity(identity)?;
    let namespace = target.namespace.unwrap_or(parsed_namespace);
    let name = target.name.unwrap_or(parsed_name);
    let internal = is_internal_namespace(materialization.inventory, namespace);
    if internal {
        if target.generated {
            super::semantic_ssa_target_support::ensure_generated_internal_function(
                identity,
                name,
                namespace,
                target.container_key,
                materialization,
            )?;
            return Ok(id);
        }
        return Err(AdapterError::new(format!(
            "Go semantic call target is not materialized: {identity:?}"
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
    )?;
    Ok(id)
}

pub(super) fn inferred_form(target: &TargetEvidence<'_>) -> CallForm {
    let identity = target.semantic_key;
    if identity.starts_with("type:") || identity.starts_with("type-expression:") {
        CallForm::Conversion
    } else if identity.starts_with("function:go:builtins:") {
        CallForm::Builtin
    } else if identity.starts_with("interface-method:") {
        CallForm::Interface
    } else if identity.starts_with("dynamic-method:") || identity.starts_with("ssa-function:") {
        CallForm::Dynamic
    } else {
        CallForm::Direct
    }
}

fn ensure_interface_target(
    identity: &str,
    evidence: &TargetEvidence<'_>,
    materialization: &mut TargetMaterialization<'_>,
) -> Result<(), AdapterError> {
    let (namespace, parsed_name) = callable_identity(identity)?;
    let internal = is_internal_namespace(materialization.inventory, namespace);
    if internal {
        let name = evidence.name.unwrap_or(parsed_name);
        let owner = evidence.owner.ok_or_else(|| {
            AdapterError::new(format!(
                "Go semantic internal interface target is missing owner: {identity:?}"
            ))
        })?;
        let span = evidence.span.ok_or_else(|| {
            AdapterError::new(format!(
                "Go semantic internal interface target is missing span: {identity:?}"
            ))
        })?;
        let container = evidence.container_key.ok_or_else(|| {
            AdapterError::new(format!(
                "Go semantic internal interface target is missing container: {identity:?}"
            ))
        })?;
        let parent = materialization.index.node_id(container)?;
        if !materialization.index.contains_node(&parent) {
            return Err(AdapterError::new(format!(
                "Go semantic internal interface container is not materialized: {container:?}"
            )));
        }
        let id = materialization.index.node_id(identity)?;
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
    evidence: TargetEvidence<'_>,
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
                id: materialization.index.node_id(identity)?,
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
            evidence.name.unwrap_or(identity),
            evidence.namespace.unwrap_or("go:ssa"),
            evidence.container_key,
            materialization,
        );
    }

    let (namespace, name) = callable_identity(identity)?;
    if is_internal_namespace(materialization.inventory, namespace) {
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

fn is_internal_namespace(inventory: &Inventory, namespace: &str) -> bool {
    inventory.modules.iter().any(|module| {
        namespace == module.path || namespace.starts_with(&format!("{}/", module.path))
    })
}

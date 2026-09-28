use crate::{AdapterError, FactRecord};

use super::{
    discovery::Inventory,
    observations::{CallForm, Span, SymbolReference},
    semantic_call_contract_targets::{ensure_callable, ensure_type_target},
    semantic_call_target_materialization::{
        ensure_callable_target, ensure_dynamic_target, ensure_interface_target,
    },
    semantic_call_target_support::{callable_identity, ensure_namespace},
    semantic_fact_index::FactIndex,
    semantic_policy::{self, TargetRepresentation},
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
    let identity = materialization
        .index
        .canonical_target_identity(target.semantic_key, target.namespace)?;
    let id = materialization.index.node_id(&identity)?;
    if materialization.index.contains_node(&id) {
        return Ok(id);
    }

    match semantic_policy::target_representation(form, &identity) {
        TargetRepresentation::Conversion => {
            ensure_type_target(
                &identity,
                materialization.inventory,
                materialization.records,
                materialization.index,
            )?;
        }
        TargetRepresentation::Builtin => {
            let (namespace, name) = callable_identity(&identity)?;
            ensure_namespace(
                "go:builtins",
                "@builtin/go",
                materialization.inventory,
                materialization.records,
                materialization.index,
            )?;
            ensure_callable(
                &identity,
                namespace,
                name,
                "@builtin/go",
                materialization.records,
                materialization.index,
            )?;
        }
        TargetRepresentation::Interface => {
            ensure_interface_target(&identity, &target, materialization)?;
        }
        TargetRepresentation::Dynamic => {
            ensure_dynamic_target(&identity, target, materialization)?;
        }
        TargetRepresentation::Closure => {
            return Err(AdapterError::new(format!(
                "Go semantic closure target is not materialized: {identity:?}"
            )));
        }
        TargetRepresentation::Callable => {
            ensure_callable_target(&identity, &target, materialization)?;
        }
    }
    Ok(id)
}

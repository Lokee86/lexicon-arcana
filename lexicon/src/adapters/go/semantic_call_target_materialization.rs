use crate::{AdapterError, NodeRecord, SourceSpan};

use super::{
    identities,
    semantic_call_contract_targets::ensure_callable,
    semantic_call_target_support::{
        SyntheticNode, callable_identity, ensure_namespace, ensure_node, namespace_path,
    },
    semantic_call_targets::{TargetEvidence, TargetMaterialization},
    semantic_facts_support::push_edge,
    semantic_identity_policy,
};

pub(super) fn ensure_callable_target(
    identity: &str,
    evidence: &TargetEvidence<'_>,
    materialization: &mut TargetMaterialization<'_>,
) -> Result<(), AdapterError> {
    let (parsed_namespace, parsed_name) = callable_identity(identity)?;
    let namespace = semantic_identity_policy::canonical_namespace(
        &materialization.inventory.modules,
        evidence.namespace.unwrap_or(parsed_namespace),
    );
    let name = evidence.name.unwrap_or(parsed_name);
    if semantic_identity_policy::is_internal_namespace(
        &materialization.inventory.modules,
        &namespace,
    ) {
        if evidence.generated {
            super::semantic_ssa_target_support::ensure_generated_internal_function(
                identity,
                name,
                &namespace,
                evidence.container_key,
                materialization,
            )?;
            return Ok(());
        }
        return Err(AdapterError::new(format!(
            "Go semantic call target is not materialized: {identity:?}"
        )));
    }

    let path = namespace_path(&namespace);
    ensure_namespace(
        &namespace,
        &path,
        materialization.inventory,
        materialization.records,
        materialization.index,
    )?;
    ensure_callable(
        identity,
        &namespace,
        name,
        &path,
        materialization.records,
        materialization.index,
    )
}

pub(super) fn ensure_interface_target(
    identity: &str,
    evidence: &TargetEvidence<'_>,
    materialization: &mut TargetMaterialization<'_>,
) -> Result<(), AdapterError> {
    let (parsed_namespace, parsed_name) = callable_identity(identity)?;
    let namespace = semantic_identity_policy::canonical_namespace(
        &materialization.inventory.modules,
        evidence.namespace.unwrap_or(parsed_namespace),
    );
    if semantic_identity_policy::is_internal_namespace(
        &materialization.inventory.modules,
        &namespace,
    ) {
        return ensure_internal_interface_target(identity, evidence, parsed_name, materialization);
    }

    let path = namespace_path(&namespace);
    ensure_namespace(
        &namespace,
        &path,
        materialization.inventory,
        materialization.records,
        materialization.index,
    )?;
    ensure_callable(
        identity,
        &namespace,
        parsed_name,
        &path,
        materialization.records,
        materialization.index,
    )
}

fn ensure_internal_interface_target(
    identity: &str,
    evidence: &TargetEvidence<'_>,
    parsed_name: &str,
    materialization: &mut TargetMaterialization<'_>,
) -> Result<(), AdapterError> {
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
    let parent = materialization.index.semantic_node_id(container)?;
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
        push_edge(
            materialization.records,
            materialization.index,
            parent,
            id,
            "defines",
            Some(owner.into()),
            Some(location),
        );
    }
    Ok(())
}

pub(super) fn ensure_dynamic_target(
    identity: &str,
    evidence: TargetEvidence<'_>,
    materialization: &mut TargetMaterialization<'_>,
) -> Result<(), AdapterError> {
    if let Some(body) = identity.strip_prefix("dynamic-method:") {
        return ensure_dynamic_method(identity, body, materialization);
    }
    if identity.starts_with("ssa-function:") {
        let namespace = semantic_identity_policy::canonical_namespace(
            &materialization.inventory.modules,
            evidence.namespace.unwrap_or("go:ssa"),
        );
        return super::semantic_ssa_target_support::ensure_synthetic_function(
            identity,
            evidence.name.unwrap_or(identity),
            &namespace,
            evidence.container_key,
            materialization,
        );
    }

    let (namespace, name) = callable_identity(identity)?;
    if semantic_identity_policy::is_internal_namespace(
        &materialization.inventory.modules,
        namespace,
    ) {
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

fn ensure_dynamic_method(
    identity: &str,
    body: &str,
    materialization: &mut TargetMaterialization<'_>,
) -> Result<(), AdapterError> {
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
    ensure_node(
        SyntheticNode {
            id: materialization.index.node_id(identity)?,
            kind: "method",
            name,
            path: "@types/go",
            namespace: "go:types",
        },
        materialization.records,
        materialization.index,
    )
}

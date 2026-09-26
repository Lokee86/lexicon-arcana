use crate::{AdapterError, NodeRecord};

use super::{
    discovery::Inventory,
    identities,
    semantic_call_target_support::{SyntheticNode, ensure_namespace, ensure_node, namespace_path},
    semantic_call_targets::TargetMaterialization,
    semantic_facts_support::push_edge,
};

pub(super) fn ensure_synthetic_function(
    identity: &str,
    name: &str,
    namespace: &str,
    container: Option<&str>,
    materialization: &mut TargetMaterialization<'_>,
) -> Result<(), AdapterError> {
    let id = materialization.index.node_id(identity)?;
    if materialization.index.contains_node(&id) {
        return Ok(());
    }

    if let Some(path) = internal_namespace_path(materialization.inventory, namespace) {
        let parent = match container {
            Some(identity) => {
                let candidate = materialization.index.node_id(identity)?;
                if materialization.index.contains_node(&candidate) {
                    candidate
                } else {
                    materialization.index.node_id(&identities::repository(
                        &materialization.inventory.repository,
                    ))?
                }
            }
            None => materialization.index.node_id(&identities::repository(
                &materialization.inventory.repository,
            ))?,
        };
        if materialization.index.push_node(
            materialization.records,
            NodeRecord {
                attributes: None,
                content_id: None,
                id: id.clone(),
                kind: "function".into(),
                name: name.into(),
                owner: None,
                path: path.clone(),
                qualified_name: format!("{path}::{name}"),
                span: None,
            },
        ) {
            let owner = materialization.index.node_owner(&parent);
            push_edge(
                materialization.records,
                materialization.index,
                parent,
                id,
                "defines",
                owner,
                None,
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
    ensure_node(
        SyntheticNode {
            id,
            kind: "function",
            name,
            path: &path,
            namespace,
        },
        materialization.records,
        materialization.index,
    )
}

pub(super) fn ensure_generated_internal_function(
    identity: &str,
    name: &str,
    namespace: &str,
    container: Option<&str>,
    materialization: &mut TargetMaterialization<'_>,
) -> Result<(), AdapterError> {
    let id = materialization.index.node_id(identity)?;
    if materialization.index.contains_node(&id) {
        return Ok(());
    }
    let path = internal_namespace_path(materialization.inventory, namespace).ok_or_else(|| {
        AdapterError::new(format!(
            "Go generated internal target is outside the repository: {identity:?}"
        ))
    })?;
    let (parsed_namespace, parsed_name) =
        super::semantic_call_target_support::callable_identity(identity)?;
    if parsed_namespace != namespace || parsed_name != name {
        return Err(AdapterError::new(format!(
            "Go generated internal target metadata does not match identity: {identity:?}"
        )));
    }
    materialization.index.push_node(
        materialization.records,
        NodeRecord {
            attributes: None,
            content_id: None,
            id: id.clone(),
            kind: identities::lexicon_kind(identity)?.into(),
            name: name.into(),
            owner: None,
            path: path.clone(),
            qualified_name: format!("{path}::{name}"),
            span: None,
        },
    );
    if let Some(container) = container.filter(|value| !value.is_empty()) {
        let parent = materialization.index.node_id(container)?;
        if materialization.index.contains_node(&parent) {
            let owner = materialization.index.node_owner(&parent);
            push_edge(
                materialization.records,
                materialization.index,
                parent,
                id,
                "defines",
                owner,
                None,
            );
        }
    }
    Ok(())
}

fn internal_namespace_path(inventory: &Inventory, namespace: &str) -> Option<String> {
    let mut selected: Option<(&str, &str)> = None;
    for module in &inventory.modules {
        if namespace != module.path && !namespace.starts_with(&format!("{}/", module.path)) {
            continue;
        }
        if selected
            .as_ref()
            .is_none_or(|(_, path)| module.path.len() > path.len())
        {
            selected = Some((module.root.as_str(), module.path.as_str()));
        }
    }
    let (root, module_path) = selected?;
    let relative = namespace
        .strip_prefix(module_path)
        .unwrap_or_default()
        .trim_start_matches('/');
    let mut path = if root == "." {
        String::new()
    } else {
        root.to_owned()
    };
    if !relative.is_empty() {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(relative);
    }
    if path.is_empty() {
        path = ".lexicon-repository".into();
    }
    Some(path)
}

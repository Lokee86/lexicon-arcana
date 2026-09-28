use crate::AdapterError;

use super::{discovery::Module, identities};

pub(super) fn canonical_identity(
    modules: &[Module],
    semantic_key: &str,
) -> Result<String, AdapterError> {
    let (kind, body) = semantic_key
        .split_once(':')
        .ok_or_else(|| AdapterError::new(format!("invalid Go semantic key {semantic_key:?}")))?;
    match kind {
        "package" => named(modules, body, semantic_key, identities::package),
        "import" => {
            let (class, path) = body.split_once(':').ok_or_else(|| {
                AdapterError::new(format!("invalid Go import semantic key {semantic_key:?}"))
            })?;
            Ok(identities::import(class, path))
        }
        "namespace" => Ok(identities::namespace(&canonical_namespace(modules, body))),
        "type" => named(modules, body, semantic_key, identities::named_type),
        "function" => named(modules, body, semantic_key, identities::function),
        "method" | "interface-method" => canonical_method(modules, kind, body, semantic_key),
        "test" => named(modules, body, semantic_key, identities::test),
        "closure" => canonical_closure(modules, body, semantic_key),
        "parameter" | "variable" | "field" | "constant" => {
            canonical_positioned(modules, kind, body, semantic_key)
        }
        "type-expression" => Ok(identities::type_expression(body)),
        "dynamic-method" => {
            let (receiver, name) = body.rsplit_once('.').ok_or_else(|| {
                AdapterError::new(format!(
                    "invalid Go dynamic method semantic key {semantic_key:?}"
                ))
            })?;
            Ok(identities::dynamic_method(receiver, name))
        }
        "ssa-function" => Err(AdapterError::new(format!(
            "Go SSA semantic key requires symbol namespace evidence: {semantic_key:?}"
        ))),
        _ => Err(AdapterError::new(format!(
            "unsupported Go semantic key kind {kind:?}"
        ))),
    }
}

pub(super) fn canonical_target_identity(
    modules: &[Module],
    semantic_key: &str,
    namespace: Option<&str>,
) -> Result<String, AdapterError> {
    if !semantic_key.starts_with("ssa-function:") {
        return canonical_identity(modules, semantic_key);
    }
    let namespace = namespace.filter(|value| !value.is_empty()).ok_or_else(|| {
        AdapterError::new(format!(
            "Go SSA target is missing namespace evidence: {semantic_key:?}"
        ))
    })?;
    let prefix = format!("ssa-function:{namespace}:");
    let remainder = semantic_key.strip_prefix(&prefix).ok_or_else(|| {
        AdapterError::new(format!(
            "Go SSA target namespace does not match semantic key: {semantic_key:?}"
        ))
    })?;
    let (display, position) = ssa_display_position(remainder);
    Ok(identities::ssa_function(
        &canonical_namespace(modules, namespace),
        display,
        position,
    ))
}

pub(super) fn is_internal_namespace(modules: &[Module], namespace: &str) -> bool {
    let namespace = canonical_namespace(modules, namespace);
    modules.iter().any(|module| {
        namespace == module.path || namespace.starts_with(&format!("{}/", module.path))
    })
}

pub(super) fn canonical_namespace(modules: &[Module], namespace: &str) -> String {
    identities::canonical_namespace(modules, namespace)
}

fn named(
    modules: &[Module],
    body: &str,
    semantic_key: &str,
    build: fn(&str, &str) -> String,
) -> Result<String, AdapterError> {
    let (namespace, name) = split_named(body, semantic_key)?;
    Ok(build(&canonical_namespace(modules, namespace), name))
}

fn canonical_method(
    modules: &[Module],
    kind: &str,
    body: &str,
    semantic_key: &str,
) -> Result<String, AdapterError> {
    let (namespace, member) = split_named(body, semantic_key)?;
    let (receiver, name) = member.rsplit_once('.').ok_or_else(|| {
        AdapterError::new(format!("invalid Go method semantic key {semantic_key:?}"))
    })?;
    let namespace = canonical_namespace(modules, namespace);
    if kind == "method" {
        Ok(identities::method(&namespace, receiver, name))
    } else {
        Ok(identities::interface_method(&namespace, receiver, name))
    }
}

fn split_named<'a>(body: &'a str, semantic_key: &str) -> Result<(&'a str, &'a str), AdapterError> {
    let (namespace, name) = body
        .rsplit_once(':')
        .ok_or_else(|| AdapterError::new(format!("invalid Go semantic key {semantic_key:?}")))?;
    if namespace.is_empty() || name.is_empty() {
        return Err(AdapterError::new(format!(
            "incomplete Go semantic key {semantic_key:?}"
        )));
    }
    Ok((namespace, name))
}

fn canonical_closure(
    modules: &[Module],
    body: &str,
    semantic_key: &str,
) -> Result<String, AdapterError> {
    let mut parts = body.rsplitn(4, ':');
    let column = parse_position(parts.next(), semantic_key, "column")?;
    let line = parse_position(parts.next(), semantic_key, "line")?;
    let owner = required_part(parts.next(), semantic_key, "owner")?;
    let namespace = required_part(parts.next(), semantic_key, "namespace")?;
    identities::closure(
        &canonical_namespace(modules, namespace),
        owner,
        line,
        column,
    )
}

fn canonical_positioned(
    modules: &[Module],
    kind: &str,
    body: &str,
    semantic_key: &str,
) -> Result<String, AdapterError> {
    let mut parts = body.rsplitn(5, ':');
    let name = required_part(parts.next(), semantic_key, "name")?;
    let column = parse_position(parts.next(), semantic_key, "column")?;
    let line = parse_position(parts.next(), semantic_key, "line")?;
    let owner = required_part(parts.next(), semantic_key, "owner")?;
    let namespace = required_part(parts.next(), semantic_key, "namespace")?;
    identities::positioned_symbol(
        kind,
        &canonical_namespace(modules, namespace),
        owner,
        line,
        column,
        name,
    )
}

fn required_part<'a>(
    value: Option<&'a str>,
    semantic_key: &str,
    label: &str,
) -> Result<&'a str, AdapterError> {
    value.filter(|value| !value.is_empty()).ok_or_else(|| {
        AdapterError::new(format!(
            "Go semantic key {semantic_key:?} is missing {label}"
        ))
    })
}

fn parse_position(
    value: Option<&str>,
    semantic_key: &str,
    label: &str,
) -> Result<u64, AdapterError> {
    let value = required_part(value, semantic_key, label)?;
    let parsed = value.parse::<u64>().map_err(|_| {
        AdapterError::new(format!(
            "Go semantic key {semantic_key:?} has invalid {label}"
        ))
    })?;
    if parsed == 0 {
        return Err(AdapterError::new(format!(
            "Go semantic key {semantic_key:?} has zero {label}"
        )));
    }
    Ok(parsed)
}

fn ssa_display_position(value: &str) -> (&str, Option<(u64, u64)>) {
    let Some((prefix, column)) = value.rsplit_once(':') else {
        return (value, None);
    };
    let Ok(column) = column.parse::<u64>() else {
        return (value, None);
    };
    let Some((display, line)) = prefix.rsplit_once(':') else {
        return (value, None);
    };
    let Ok(line) = line.parse::<u64>() else {
        return (value, None);
    };
    (display, Some((line, column)))
}

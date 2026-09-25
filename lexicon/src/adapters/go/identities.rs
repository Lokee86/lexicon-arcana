#![allow(dead_code)] // Identity constructors are the stable seam consumed by Go migration phases 6-11.

use crate::{AdapterError, node_id as lexicon_node_id};

use super::discovery::Module;

pub(crate) fn repository(value: &str) -> String {
    format!("repository:{value}")
}

pub(crate) fn directory(path: &str) -> String {
    format!("directory:{path}")
}

pub(crate) fn file(path: &str) -> String {
    format!("file:{path}")
}

pub(crate) fn package(namespace: &str, name: &str) -> String {
    format!("package:{namespace}:{name}")
}

pub(crate) fn import(class: &str, path: &str) -> String {
    format!("import:{class}:{path}")
}

pub(crate) fn namespace(value: &str) -> String {
    format!("namespace:{value}")
}

pub(crate) fn named_type(namespace: &str, name: &str) -> String {
    format!("type:{namespace}:{name}")
}

pub(crate) fn function(namespace: &str, name: &str) -> String {
    format!("function:{namespace}:{name}")
}

pub(crate) fn method(namespace: &str, receiver: &str, name: &str) -> String {
    format!("method:{namespace}:{receiver}.{name}")
}

pub(crate) fn interface_method(namespace: &str, interface: &str, name: &str) -> String {
    format!("interface-method:{namespace}:{interface}.{name}")
}

pub(crate) fn test(namespace: &str, name: &str) -> String {
    format!("test:{namespace}:{name}")
}

pub(crate) fn closure(
    namespace: &str,
    owner: &str,
    line: u64,
    column: u64,
) -> Result<String, AdapterError> {
    validate_owner(owner)?;
    Ok(format!("closure:{namespace}:{owner}:{line}:{column}"))
}

pub(crate) fn positioned_symbol(
    kind: &str,
    namespace: &str,
    owner: &str,
    line: u64,
    column: u64,
    name: &str,
) -> Result<String, AdapterError> {
    if !matches!(kind, "parameter" | "variable" | "field" | "constant") {
        return Err(AdapterError::new(format!(
            "unsupported positioned Go identity kind {kind:?}"
        )));
    }
    validate_owner(owner)?;
    Ok(format!("{kind}:{namespace}:{owner}:{line}:{column}:{name}"))
}

pub(crate) fn capture(closure_id: &str, index: usize, name: &str) -> String {
    format!("capture:{closure_id}:{index}:{name}")
}

pub(crate) fn ssa_function(namespace: &str, display: &str, position: Option<(u64, u64)>) -> String {
    let mut identity = format!("ssa-function:{namespace}:{display}");
    if let Some((line, column)) = position {
        identity.push_str(&format!(":{line}:{column}"));
    }
    identity
}

pub(crate) fn builtin(name: &str) -> String {
    function("go:builtins", name)
}

pub(crate) fn type_expression(name: &str) -> String {
    format!("type-expression:{name}")
}

pub(crate) fn dynamic_method(receiver: &str, name: &str) -> String {
    format!("dynamic-method:{receiver}.{name}")
}

pub(crate) fn canonical_namespace(modules: &[Module], namespace: &str) -> String {
    let Some(base) = namespace.strip_suffix("_test") else {
        return namespace.to_owned();
    };
    if modules
        .iter()
        .any(|module| base == module.path || base.starts_with(&format!("{}/", module.path)))
    {
        return base.to_owned();
    }
    namespace.to_owned()
}

pub(crate) fn node_id(identity: &str) -> Result<String, AdapterError> {
    let kind = lexicon_kind(identity)?;
    validate_identity(identity)?;
    Ok(lexicon_node_id("go", kind, identity))
}

pub(crate) fn node_id_for_kind(
    identity: &str,
    expected_kind: &str,
) -> Result<String, AdapterError> {
    let actual = lexicon_kind(identity)?;
    if actual != expected_kind {
        return Err(AdapterError::new(format!(
            "Go identity {identity:?} maps to {actual:?}, expected {expected_kind:?}"
        )));
    }
    validate_identity(identity)?;
    Ok(lexicon_node_id("go", actual, identity))
}

pub(crate) fn lexicon_kind(identity: &str) -> Result<&'static str, AdapterError> {
    let prefix = identity
        .split_once(':')
        .map(|(prefix, _)| prefix)
        .ok_or_else(|| AdapterError::new(format!("invalid Go identity {identity:?}")))?;
    match prefix {
        "repository" => Ok("repository"),
        "directory" => Ok("directory"),
        "file" => Ok("file"),
        "package" => Ok("module"),
        "import" => Ok("import"),
        "namespace" => Ok("namespace"),
        "type" | "type-expression" => Ok("type"),
        "function" | "closure" | "ssa-function" => Ok("function"),
        "method" | "interface-method" | "dynamic-method" => Ok("method"),
        "test" => Ok("test"),
        "parameter" => Ok("parameter"),
        "variable" | "capture" => Ok("variable"),
        "field" => Ok("field"),
        "constant" => Ok("constant"),
        _ => Err(AdapterError::new(format!(
            "unknown Go identity prefix {prefix:?}"
        ))),
    }
}

fn validate_identity(identity: &str) -> Result<(), AdapterError> {
    let (_, body) = identity
        .split_once(':')
        .ok_or_else(|| AdapterError::new(format!("invalid Go identity {identity:?}")))?;
    if body.is_empty() || identity.contains(['\0', '\n', '\r', '\\']) || identity.contains(":/") {
        return Err(AdapterError::new(format!(
            "non-canonical Go identity {identity:?}"
        )));
    }
    Ok(())
}

fn validate_owner(owner: &str) -> Result<(), AdapterError> {
    if owner.is_empty()
        || owner.starts_with('/')
        || owner.contains('\\')
        || owner
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(AdapterError::new(format!(
            "Go identity owner {owner:?} is not repository-relative"
        )));
    }
    Ok(())
}

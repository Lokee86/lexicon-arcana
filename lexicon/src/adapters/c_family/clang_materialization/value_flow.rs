use std::collections::HashMap;

use crate::AdapterError;

use super::{
    super::{
        clang_protocol::{
            FileObservation, SemanticAccessObservation as ProtocolAccess,
            SemanticPointerBindingObservation as ProtocolBinding,
        },
        model::{SemanticAccessObservation, SemanticPointerBindingObservation},
    },
    references::ReferenceIndex,
};

pub(super) fn materialize(
    file: &FileObservation,
    local: &HashMap<String, String>,
    references: &ReferenceIndex,
) -> Result<
    (
        Vec<SemanticPointerBindingObservation>,
        Vec<SemanticAccessObservation>,
    ),
    AdapterError,
> {
    let bindings = file
        .pointer_bindings
        .iter()
        .map(|value| pointer_binding(&file.path, value, references))
        .collect::<Result<Vec<_>, _>>()?;
    let accesses = file
        .accesses
        .iter()
        .map(|value| access(&file.path, value, local, references))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((bindings, accesses))
}

fn pointer_binding(
    path: &str,
    value: &ProtocolBinding,
    references: &ReferenceIndex,
) -> Result<SemanticPointerBindingObservation, AdapterError> {
    let pointer_id = references.resolve(&value.pointer, path);
    if pointer_id.is_empty() {
        return Err(AdapterError::new(format!(
            "C-family Clang pointer {:?} is not materialized in {path}",
            value.pointer.qualified_name
        )));
    }
    Ok(SemanticPointerBindingObservation {
        pointer_id,
        target_id: references.resolve(&value.target, path),
    })
}

fn access(
    path: &str,
    value: &ProtocolAccess,
    local: &HashMap<String, String>,
    references: &ReferenceIndex,
) -> Result<SemanticAccessObservation, AdapterError> {
    if !matches!(value.relation.as_str(), "reads" | "writes") {
        return Err(AdapterError::new(format!(
            "unsupported C-family Clang access relation {:?}",
            value.relation
        )));
    }
    let source_id = local
        .get(&value.source_compiler_id)
        .cloned()
        .unwrap_or_else(|| references.resolve_compiler_id(&value.source_compiler_id, path));
    if source_id.is_empty() {
        return Err(AdapterError::new(format!(
            "C-family Clang access source {:?} is not materialized",
            value.source_compiler_id
        )));
    }
    Ok(SemanticAccessObservation {
        source_id,
        target_id: references.resolve(&value.target, path),
        path: path.into(),
        relation: value.relation.clone(),
        span: value.span.clone(),
    })
}

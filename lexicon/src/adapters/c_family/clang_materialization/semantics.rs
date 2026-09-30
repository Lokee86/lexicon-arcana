use std::collections::HashMap;

use crate::AdapterError;

use super::{
    super::{
        clang_protocol::{
            FileObservation, SemanticArgumentObservation as ProtocolArgument,
            SemanticCallObservation as ProtocolCall,
            SemanticRelationshipObservation as ProtocolRelationship,
        },
        model::{
            SemanticArgumentObservation, SemanticCallForm, SemanticCallObservation,
            SemanticCallResolution, SemanticRelationshipKind, SemanticRelationshipObservation,
        },
    },
    references::{IdentityMaps, ReferenceIndex},
};

pub(super) fn materialize(
    file: &FileObservation,
    ids: &IdentityMaps,
    references: &ReferenceIndex,
) -> Result<
    (
        Vec<SemanticRelationshipObservation>,
        Vec<SemanticCallObservation>,
    ),
    AdapterError,
> {
    let local = ids
        .get(&file.path)
        .expect("C-family Clang local identity map");
    let relationships = file
        .relationships
        .iter()
        .map(|value| relationship(&file.path, value, local, references))
        .collect::<Result<Vec<_>, _>>()?;
    let calls = file
        .calls
        .iter()
        .map(|value| call(&file.path, value, local, references))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((relationships, calls))
}

fn relationship(
    path: &str,
    value: &ProtocolRelationship,
    local: &HashMap<String, String>,
    references: &ReferenceIndex,
) -> Result<SemanticRelationshipObservation, AdapterError> {
    let kind = match value.kind.as_str() {
        "extends" => SemanticRelationshipKind::Extends,
        "overrides" => SemanticRelationshipKind::Overrides,
        other => {
            return Err(AdapterError::new(format!(
                "unsupported C-family Clang relationship kind {other:?}"
            )));
        }
    };
    let source_id = local
        .get(&value.source_compiler_id)
        .cloned()
        .unwrap_or_else(|| references.resolve_compiler_id(&value.source_compiler_id, path));
    if source_id.is_empty() {
        return Err(AdapterError::new(format!(
            "C-family Clang relationship source {:?} is not materialized",
            value.source_compiler_id
        )));
    }
    Ok(SemanticRelationshipObservation {
        source_id,
        target_id: references.resolve(&value.target, path),
        target_name: value.target.qualified_name.clone(),
        external: value.target.external,
        path: path.into(),
        expression: value.expression.clone(),
        kind,
        span: value.span.clone(),
    })
}

fn call(
    path: &str,
    value: &ProtocolCall,
    local: &HashMap<String, String>,
    references: &ReferenceIndex,
) -> Result<SemanticCallObservation, AdapterError> {
    let source_id = local
        .get(&value.source_compiler_id)
        .cloned()
        .unwrap_or_else(|| references.resolve_compiler_id(&value.source_compiler_id, path));
    if source_id.is_empty() {
        return Err(AdapterError::new(format!(
            "C-family Clang call source {:?} is not materialized",
            value.source_compiler_id
        )));
    }
    let form = match value.form.as_str() {
        "direct" => SemanticCallForm::Direct,
        "member" => SemanticCallForm::Member,
        "constructor" => SemanticCallForm::Constructor,
        "operator" => SemanticCallForm::Operator,
        "destructor" => SemanticCallForm::Destructor,
        other => {
            return Err(AdapterError::new(format!(
                "unsupported C-family Clang call form {other:?}"
            )));
        }
    };
    let resolution = match value.resolution.as_str() {
        "resolved" => SemanticCallResolution::Resolved,
        "ambiguous" => SemanticCallResolution::Ambiguous,
        "missing" => SemanticCallResolution::Missing,
        "dependent" => SemanticCallResolution::Dependent,
        "indirect" => SemanticCallResolution::Indirect,
        other => {
            return Err(AdapterError::new(format!(
                "unsupported C-family Clang call resolution {other:?}"
            )));
        }
    };
    let target_id = value
        .target
        .as_ref()
        .map(|target| references.resolve(target, path))
        .unwrap_or_default();
    let target_name = value
        .target
        .as_ref()
        .map(|target| target.qualified_name.clone())
        .unwrap_or_default();
    let mut candidate_ids = value
        .candidates
        .iter()
        .map(|candidate| references.resolve(candidate, path))
        .filter(|candidate| !candidate.is_empty())
        .collect::<Vec<_>>();
    candidate_ids.sort();
    candidate_ids.dedup();

    let receiver_type_id = value
        .receiver_type
        .as_ref()
        .map(|target| references.resolve(target, path))
        .unwrap_or_default();
    let receiver_type_name = value
        .receiver_type
        .as_ref()
        .filter(|target| !target.qualified_name.is_empty())
        .map(|target| target.qualified_name.clone())
        .unwrap_or_else(|| value.receiver_type_name.clone());
    let callee_value_id = value
        .callee_value
        .as_ref()
        .map(|target| references.resolve(target, path))
        .unwrap_or_default();
    let external_candidate_count = value
        .candidates
        .iter()
        .filter(|candidate| candidate.external)
        .count()
        + value.target.iter().filter(|target| target.external).count();

    Ok(SemanticCallObservation {
        source_id,
        path: path.into(),
        expression: value.expression.clone(),
        form,
        resolution,
        dispatch: if value.virtual_dispatch {
            "virtual".into()
        } else {
            "static".into()
        },
        overload_selected: value.overload_selected,
        macro_expanded: value.macro_expanded,
        target_id,
        target_name,
        candidate_ids,
        compiler_candidate_count: value.compiler_candidate_count,
        external_candidate_count,
        receiver_type_id,
        receiver_type: receiver_type_name,
        callee_value_id,
        arguments: value
            .arguments
            .iter()
            .map(|argument| materialize_argument(argument, path, references))
            .collect(),
        span: value.span.clone(),
    })
}

fn materialize_argument(
    value: &ProtocolArgument,
    path: &str,
    references: &ReferenceIndex,
) -> SemanticArgumentObservation {
    SemanticArgumentObservation {
        expression: value.expression.clone(),
        value_id: value
            .value
            .as_ref()
            .map(|target| references.resolve(target, path))
            .unwrap_or_default(),
        callable_id: value
            .callable
            .as_ref()
            .map(|target| references.resolve(target, path))
            .unwrap_or_default(),
    }
}

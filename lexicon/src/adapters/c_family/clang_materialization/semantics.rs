use std::collections::HashMap;

use crate::AdapterError;

use super::super::{
    clang_protocol::{
        FileObservation, SemanticCallObservation as ProtocolCall,
        SemanticRelationshipObservation as ProtocolRelationship, SymbolReferenceObservation,
    },
    model::{
        SemanticCallForm, SemanticCallObservation, SemanticCallResolution,
        SemanticRelationshipKind, SemanticRelationshipObservation,
    },
};

pub(super) type IdentityMaps = HashMap<String, HashMap<String, String>>;

#[derive(Debug)]
pub(super) struct ReferenceIndex {
    by_compiler: HashMap<String, Vec<ReferenceCandidate>>,
}

#[derive(Debug, Clone)]
struct ReferenceCandidate {
    path: String,
    id: String,
    definition: bool,
}

impl ReferenceIndex {
    pub(super) fn new(files: &[&FileObservation], ids: &IdentityMaps) -> Self {
        let mut by_compiler = HashMap::<String, Vec<ReferenceCandidate>>::new();
        for file in files {
            let Some(file_ids) = ids.get(&file.path) else {
                continue;
            };
            for declaration in &file.declarations {
                let Some(id) = file_ids.get(&declaration.compiler_id) else {
                    continue;
                };
                by_compiler
                    .entry(declaration.compiler_id.clone())
                    .or_default()
                    .push(ReferenceCandidate {
                        path: file.path.clone(),
                        id: id.clone(),
                        definition: declaration.definition,
                    });
            }
        }
        for values in by_compiler.values_mut() {
            values.sort_by(|left, right| {
                (!left.definition, left.path.as_str(), left.id.as_str()).cmp(&(
                    !right.definition,
                    right.path.as_str(),
                    right.id.as_str(),
                ))
            });
            values.dedup_by(|left, right| left.id == right.id);
        }
        Self { by_compiler }
    }

    fn resolve(&self, reference: &SymbolReferenceObservation, source_path: &str) -> String {
        if reference.external {
            return String::new();
        }
        let Some(candidates) = self.by_compiler.get(&reference.compiler_id) else {
            return String::new();
        };
        if let Some(value) = candidates.iter().find(|value| value.path == source_path) {
            return value.id.clone();
        }
        if let Some(value) = candidates.iter().find(|value| value.definition) {
            return value.id.clone();
        }
        if !reference.path.is_empty()
            && let Some(value) = candidates.iter().find(|value| value.path == reference.path)
        {
            return value.id.clone();
        }
        candidates
            .first()
            .map(|value| value.id.clone())
            .unwrap_or_default()
    }
}

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
        .ok_or_else(|| {
            AdapterError::new(format!(
                "C-family Clang relationship source {:?} is not materialized in {path}",
                value.source_compiler_id
            ))
        })?;
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
        .ok_or_else(|| {
            AdapterError::new(format!(
                "C-family Clang call source {:?} is not materialized in {path}",
                value.source_compiler_id
            ))
        })?;
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
        target_id,
        target_name,
        candidate_ids,
        compiler_candidate_count: value.compiler_candidate_count,
        external_candidate_count,
        receiver_type_id,
        receiver_type: value.receiver_type_name.clone(),
        argument_expressions: value.arguments.clone(),
        span: value.span.clone(),
    })
}

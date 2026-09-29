use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::FactRecord;

use super::{
    model::{
        RepositoryModel, SemanticCallObservation, SemanticCallResolution, SemanticRelationshipKind,
    },
    resolution::DeclarationIndex,
    semantic_call_records::{push_edge, push_unresolved},
    semantic_pointer_index::SemanticPointerIndex,
};

pub(super) fn add(
    model: &RepositoryModel,
    declarations: &DeclarationIndex<'_>,
    records: &mut Vec<FactRecord>,
) {
    let overrides = override_index(model);
    let pointers = SemanticPointerIndex::build(model, declarations);
    for file in &model.files {
        for observation in &file.semantic_calls {
            emit(observation, declarations, &pointers, &overrides, records);
        }
    }
}

fn emit(
    observation: &SemanticCallObservation,
    declarations: &DeclarationIndex<'_>,
    pointers: &SemanticPointerIndex,
    overrides: &BTreeMap<String, Vec<String>>,
    records: &mut Vec<FactRecord>,
) {
    match observation.resolution {
        SemanticCallResolution::Resolved if !observation.target_id.is_empty() => {
            emit_resolved(observation, declarations, overrides, records);
        }
        SemanticCallResolution::Resolved => push_unresolved(
            observation,
            if observation.external_candidate_count > 0 {
                "external-target"
            } else {
                "missing-target"
            },
            records,
        ),
        SemanticCallResolution::Ambiguous => {
            let mut candidates = observation.candidate_ids.clone();
            candidates.sort();
            candidates.dedup();
            for target in &candidates {
                push_edge(
                    observation,
                    target,
                    "possible-calls",
                    observation.compiler_candidate_count,
                    None,
                    records,
                );
            }
            push_unresolved(observation, "ambiguous-target", records);
        }
        SemanticCallResolution::Indirect => {
            let targets = pointers.targets(&observation.callee_value_id);
            for target in &targets {
                push_edge(
                    observation,
                    target,
                    "possible-calls",
                    targets.len(),
                    Some(&observation.callee_value_id),
                    records,
                );
            }
            push_unresolved(observation, "dynamic-target", records);
        }
        SemanticCallResolution::Missing => push_unresolved(
            observation,
            if observation.external_candidate_count > 0 {
                "external-target"
            } else {
                "missing-target"
            },
            records,
        ),
        SemanticCallResolution::Dependent => {
            push_unresolved(observation, "dynamic-target", records);
        }
    }
}

fn emit_resolved(
    observation: &SemanticCallObservation,
    declarations: &DeclarationIndex<'_>,
    overrides: &BTreeMap<String, Vec<String>>,
    records: &mut Vec<FactRecord>,
) {
    if observation.dispatch == "virtual" {
        let targets = virtual_targets(&observation.target_id, overrides);
        for target in &targets {
            push_edge(
                observation,
                target,
                "possible-calls",
                targets.len(),
                None,
                records,
            );
        }
        push_unresolved(observation, "dynamic-target", records);
        return;
    }

    push_edge(
        observation,
        &observation.target_id,
        "calls",
        1,
        None,
        records,
    );
    super::semantic_dataflow_facts::add_passes_to(
        declarations,
        observation,
        &observation.target_id,
        records,
    );
}

fn override_index(model: &RepositoryModel) -> BTreeMap<String, Vec<String>> {
    let mut result = BTreeMap::<String, Vec<String>>::new();
    for file in &model.files {
        for relationship in &file.semantic_relationships {
            if relationship.kind == SemanticRelationshipKind::Overrides
                && !relationship.target_id.is_empty()
            {
                result
                    .entry(relationship.target_id.clone())
                    .or_default()
                    .push(relationship.source_id.clone());
            }
        }
    }
    for values in result.values_mut() {
        values.sort();
        values.dedup();
    }
    result
}

fn virtual_targets(target: &str, overrides: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([target.to_owned()]);
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current.clone()) {
            continue;
        }
        if let Some(children) = overrides.get(&current) {
            queue.extend(children.iter().cloned());
        }
    }
    seen.into_iter().collect()
}

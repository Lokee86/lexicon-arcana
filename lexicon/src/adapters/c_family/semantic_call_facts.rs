use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde_json::json;

use crate::{EdgeRecord, FactRecord, UnresolvedRecord};

use super::model::{
    RepositoryModel, SemanticCallForm, SemanticCallObservation, SemanticCallResolution,
    SemanticRelationshipKind,
};

pub(super) fn add(model: &RepositoryModel, records: &mut Vec<FactRecord>) {
    let overrides = override_index(model);
    for file in &model.files {
        for observation in &file.semantic_calls {
            emit(observation, &overrides, records);
        }
    }
}

fn emit(
    observation: &SemanticCallObservation,
    overrides: &BTreeMap<String, Vec<String>>,
    records: &mut Vec<FactRecord>,
) {
    match observation.resolution {
        SemanticCallResolution::Resolved if !observation.target_id.is_empty() => {
            if observation.dispatch == "virtual" {
                let targets = virtual_targets(&observation.target_id, overrides);
                for target in &targets {
                    push_edge(
                        observation,
                        target,
                        "possible-calls",
                        targets.len(),
                        records,
                    );
                }
                push_unresolved(observation, "dynamic-target", records);
            } else {
                push_edge(observation, &observation.target_id, "calls", 1, records);
            }
        }
        SemanticCallResolution::Resolved => {
            push_unresolved(
                observation,
                if observation.external_candidate_count > 0 {
                    "external-target"
                } else {
                    "missing-target"
                },
                records,
            );
        }
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
                    records,
                );
            }
            push_unresolved(observation, "ambiguous-target", records);
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
        SemanticCallResolution::Dependent | SemanticCallResolution::Indirect => {
            push_unresolved(observation, "dynamic-target", records);
        }
    }
}

fn push_edge(
    observation: &SemanticCallObservation,
    target: &str,
    relation: &str,
    candidate_count: usize,
    records: &mut Vec<FactRecord>,
) {
    let mut evidence = vec![
        "clang".to_owned(),
        format!("clang-{}", form_name(observation.form)),
    ];
    if observation.dispatch == "virtual" {
        evidence.push("clang-virtual-dispatch".into());
    }
    records.push(FactRecord::Edge(EdgeRecord {
        attributes: Some(json!({
            "argument_count": observation.argument_expressions.len(),
            "candidate_count": candidate_count.max(1),
            "compiler_candidate_count": observation.compiler_candidate_count,
            "dispatch": observation.dispatch,
            "overload_selected": observation.overload_selected,
            "evidence": evidence,
            "receiver_type": observation.receiver_type,
            "receiver_type_id": observation.receiver_type_id,
            "resolution": if relation == "calls" { "definite" } else { "possible" },
        })),
        owner: Some(observation.path.clone()),
        relation: relation.into(),
        source: observation.source_id.clone(),
        span: Some(observation.span.clone()),
        target: target.into(),
    }));
}

fn push_unresolved(
    observation: &SemanticCallObservation,
    reason: &str,
    records: &mut Vec<FactRecord>,
) {
    records.push(FactRecord::Unresolved(UnresolvedRecord {
        attributes: Some(json!({
            "compiler_candidate_count": observation.compiler_candidate_count,
            "dispatch": observation.dispatch,
            "form": form_name(observation.form),
            "receiver_type": observation.receiver_type,
        })),
        candidate_name: (!observation.target_name.is_empty())
            .then(|| observation.target_name.clone()),
        candidate_namespace: None,
        expression: observation.expression.clone(),
        owner: Some(observation.path.clone()),
        reason: reason.into(),
        relation: "calls".into(),
        source: observation.source_id.clone(),
        span: Some(observation.span.clone()),
    }));
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

fn form_name(form: SemanticCallForm) -> &'static str {
    match form {
        SemanticCallForm::Direct => "direct",
        SemanticCallForm::Member => "member",
        SemanticCallForm::Constructor => "constructor",
        SemanticCallForm::Operator => "operator",
        SemanticCallForm::Destructor => "destructor",
    }
}

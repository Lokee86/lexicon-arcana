use super::{
    call_candidates::{direct_qualified_types, explicit_qualifier, has_callable, resolve},
    indirect_calls::{IndirectCallIndex, resolve_pointer_declarations},
    model::CallObservation,
    resolution::DeclarationIndex,
};
use crate::{EdgeRecord, FactRecord, UnresolvedRecord};
use serde_json::json;

pub fn add(model: &super::model::RepositoryModel, records: &mut Vec<FactRecord>) {
    let index = DeclarationIndex::new(model);
    let indirect = IndirectCallIndex::build(model, &index);
    for file in &model.files {
        for observation in &file.calls {
            resolve_call(&index, &indirect, observation, records);
        }
    }
}

fn resolve_call(
    index: &DeclarationIndex<'_>,
    indirect: &IndirectCallIndex<'_>,
    observation: &CallObservation,
    records: &mut Vec<FactRecord>,
) {
    if super::macro_facts::try_resolve(index, indirect, observation, records) {
        return;
    }
    let mut resolution = resolve(index, observation);
    if resolution.candidates.len() == 1 {
        add_edge(
            observation,
            resolution.candidates[0],
            "calls",
            &resolution,
            records,
        );
        super::dataflow::add_passes_to(index, observation, resolution.candidates[0], records);
        return;
    }
    if resolution.candidates.len() > 1 {
        resolution = resolution.prune(observation.arguments.len());
        if resolution.candidates.len() == 1 {
            add_edge(
                observation,
                resolution.candidates[0],
                "calls",
                &resolution,
                records,
            );
            super::dataflow::add_passes_to(index, observation, resolution.candidates[0], records);
            return;
        }
        for candidate in &resolution.candidates {
            add_edge(
                observation,
                candidate,
                "possible-calls",
                &resolution,
                records,
            );
        }
        add_unresolved(observation, "ambiguous-target", records);
        return;
    }

    let pointers = resolve_pointer_declarations(index, observation);
    if !pointers.is_empty() {
        let targets = indirect.targets(&pointers);
        if !targets.is_empty() {
            let via = pointers
                .iter()
                .map(|pointer| pointer.id.clone())
                .collect::<Vec<_>>();
            for target in &targets {
                records.push(FactRecord::Edge(EdgeRecord {
                    attributes: Some(json!({
                        "candidate_count": targets.len(),
                        "evidence": ["function-pointer"],
                        "indirect": "function-pointer",
                        "resolution": "possible",
                        "via": via,
                    })),
                    owner: Some(observation.path.clone()),
                    relation: "possible-calls".into(),
                    source: observation.source_id.clone(),
                    span: Some(observation.span.clone()),
                    target: target.id.clone(),
                }));
            }
        }
        add_unresolved(observation, "dynamic-target", records);
        return;
    }

    let reason = if explicit_qualifier(&observation.candidate).is_some_and(|qualifier| {
        direct_qualified_types(index, &qualifier, &observation.path).is_empty()
    }) || !has_callable(index, &observation.candidate)
    {
        "external-target"
    } else {
        "missing-target"
    };
    add_unresolved(observation, reason, records);
}

fn add_edge(
    observation: &CallObservation,
    target: &super::model::Declaration,
    relation: &str,
    resolution: &super::call_candidates::CallCandidateResolution<'_>,
    records: &mut Vec<FactRecord>,
) {
    records.push(FactRecord::Edge(EdgeRecord {
        attributes: Some(json!({
            "candidate_count": resolution.candidates.len(),
            "evidence": resolution.evidence,
            "resolution": if relation == "calls" { "definite" } else { "possible" },
        })),
        owner: Some(observation.path.clone()),
        relation: relation.into(),
        source: observation.source_id.clone(),
        span: Some(observation.span.clone()),
        target: target.id.clone(),
    }));
}

fn add_unresolved(observation: &CallObservation, reason: &str, records: &mut Vec<FactRecord>) {
    records.push(FactRecord::Unresolved(UnresolvedRecord {
        attributes: None,
        candidate_name: None,
        candidate_namespace: None,
        expression: observation.expression.clone(),
        owner: Some(observation.path.clone()),
        reason: reason.into(),
        relation: "calls".into(),
        source: observation.source_id.clone(),
        span: Some(observation.span.clone()),
    }));
}

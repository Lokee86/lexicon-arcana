use super::{
    model::{InheritanceObservation, RepositoryModel},
    resolution::{DeclarationIndex, resolution_reason},
};
use crate::{EdgeRecord, FactRecord, UnresolvedRecord};

pub fn add(model: &RepositoryModel, records: &mut Vec<FactRecord>) {
    let index = DeclarationIndex::new(model);
    for file in &model.files {
        for observation in &file.inheritance {
            resolve_inheritance(&index, observation, records);
        }
    }
}

fn resolve_inheritance(
    index: &DeclarationIndex<'_>,
    observation: &InheritanceObservation,
    records: &mut Vec<FactRecord>,
) {
    let candidates = index.resolve(
        &observation.candidate,
        &observation.source_scope,
        &observation.path,
        |declaration| declaration.kind == "type",
    );

    if candidates.len() == 1 {
        records.push(FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner: Some(observation.path.clone()),
            relation: "extends".into(),
            source: observation.source_id.clone(),
            span: Some(observation.span.clone()),
            target: candidates[0].id.clone(),
        }));
        return;
    }

    records.push(FactRecord::Unresolved(UnresolvedRecord {
        attributes: None,
        candidate_name: None,
        candidate_namespace: None,
        expression: observation.expression.clone(),
        owner: Some(observation.path.clone()),
        reason: resolution_reason(&candidates).into(),
        relation: "extends".into(),
        source: observation.source_id.clone(),
        span: Some(observation.span.clone()),
    }));
}

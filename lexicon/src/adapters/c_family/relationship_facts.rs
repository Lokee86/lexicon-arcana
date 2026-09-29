use super::{
    model::{
        InheritanceObservation, RepositoryModel, SemanticRelationshipKind,
        SemanticRelationshipObservation,
    },
    resolution::{DeclarationIndex, resolution_reason},
};
use crate::{EdgeRecord, FactRecord, UnresolvedRecord};

pub fn add(model: &RepositoryModel, records: &mut Vec<FactRecord>) {
    let index = DeclarationIndex::new(model);
    for file in &model.files {
        for observation in &file.semantic_relationships {
            add_semantic_relationship(observation, records);
        }
        for observation in &file.inheritance {
            resolve_inheritance(&index, observation, records);
        }
    }
}

fn add_semantic_relationship(
    observation: &SemanticRelationshipObservation,
    records: &mut Vec<FactRecord>,
) {
    let relation = match observation.kind {
        SemanticRelationshipKind::Extends => "extends",
        SemanticRelationshipKind::Overrides => "overrides",
    };
    if !observation.target_id.is_empty() {
        records.push(FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner: Some(observation.path.clone()),
            relation: relation.into(),
            source: observation.source_id.clone(),
            span: Some(observation.span.clone()),
            target: observation.target_id.clone(),
        }));
        return;
    }

    records.push(FactRecord::Unresolved(UnresolvedRecord {
        attributes: None,
        candidate_name: (!observation.target_name.is_empty())
            .then(|| observation.target_name.clone()),
        candidate_namespace: None,
        expression: observation.expression.clone(),
        owner: Some(observation.path.clone()),
        reason: if observation.external {
            "external-target"
        } else {
            "missing-target"
        }
        .into(),
        relation: relation.into(),
        source: observation.source_id.clone(),
        span: Some(observation.span.clone()),
    }));
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

use super::model::{RepositoryModel, SemanticRelationshipKind, SemanticRelationshipObservation};
use crate::{EdgeRecord, FactRecord, UnresolvedRecord};

pub fn add(model: &RepositoryModel, records: &mut Vec<FactRecord>) {
    for file in &model.files {
        for observation in &file.semantic_relationships {
            add_semantic_relationship(observation, records);
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

use crate::{EdgeRecord, FactRecord};

use super::{
    model::{RepositoryModel, SemanticCallObservation},
    resolution::DeclarationIndex,
};
use serde_json::json;

pub(super) fn add_accesses(model: &RepositoryModel, records: &mut Vec<FactRecord>) {
    for file in &model.files {
        for observation in &file.semantic_accesses {
            if observation.target_id.is_empty() {
                continue;
            }
            records.push(FactRecord::Edge(EdgeRecord {
                attributes: None,
                owner: Some(observation.path.clone()),
                relation: observation.relation.clone(),
                source: observation.source_id.clone(),
                span: Some(observation.span.clone()),
                target: observation.target_id.clone(),
            }));
        }
    }
}

pub(super) fn add_passes_to(
    declarations: &DeclarationIndex<'_>,
    observation: &SemanticCallObservation,
    target_id: &str,
    records: &mut Vec<FactRecord>,
) {
    let Some(parameters) = declarations.by_callable_parameters.get(target_id) else {
        return;
    };
    for (argument_index, argument) in observation.arguments.iter().enumerate() {
        if argument.value_id.is_empty() {
            continue;
        }
        let Some(parameter) = parameters.iter().find(|parameter| {
            parameter
                .attributes
                .get("index")
                .and_then(|value| value.as_u64())
                == Some(argument_index as u64)
        }) else {
            continue;
        };
        records.push(FactRecord::Edge(EdgeRecord {
            attributes: Some(json!({
                "argument_index": argument_index,
                "expression": argument.expression,
                "via_call": target_id,
            })),
            owner: Some(observation.path.clone()),
            relation: "passes-to".into(),
            source: argument.value_id.clone(),
            span: Some(observation.span.clone()),
            target: parameter.id.clone(),
        }));
    }
}

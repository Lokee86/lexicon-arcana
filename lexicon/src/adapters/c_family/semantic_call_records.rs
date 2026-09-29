use serde_json::json;

use crate::{EdgeRecord, FactRecord, UnresolvedRecord};

use super::model::{SemanticCallForm, SemanticCallObservation};

pub(super) fn push_edge(
    observation: &SemanticCallObservation,
    target: &str,
    relation: &str,
    candidate_count: usize,
    pointer_via: Option<&str>,
    records: &mut Vec<FactRecord>,
) {
    let mut evidence = vec![
        "clang".to_owned(),
        format!("clang-{}", form_name(observation.form)),
    ];
    if observation.dispatch == "virtual" {
        evidence.push("clang-virtual-dispatch".into());
    }
    if observation.macro_expanded {
        evidence.push("clang-macro-expansion".into());
    }
    if pointer_via.is_some() {
        evidence.push("function-pointer".into());
    }

    let mut attributes = serde_json::Map::from_iter([
        ("argument_count".into(), json!(observation.arguments.len())),
        ("candidate_count".into(), json!(candidate_count.max(1))),
        (
            "compiler_candidate_count".into(),
            json!(observation.compiler_candidate_count),
        ),
        ("dispatch".into(), json!(observation.dispatch)),
        ("evidence".into(), json!(evidence)),
        (
            "overload_selected".into(),
            json!(observation.overload_selected),
        ),
        ("receiver_type".into(), json!(observation.receiver_type)),
        (
            "receiver_type_id".into(),
            json!(observation.receiver_type_id),
        ),
        (
            "resolution".into(),
            json!(if relation == "calls" {
                "definite"
            } else {
                "possible"
            }),
        ),
    ]);
    if let Some(pointer) = pointer_via {
        attributes.insert("pointer_via".into(), json!([pointer]));
    }

    records.push(FactRecord::Edge(EdgeRecord {
        attributes: Some(serde_json::Value::Object(attributes)),
        owner: Some(observation.path.clone()),
        relation: relation.into(),
        source: observation.source_id.clone(),
        span: Some(observation.span.clone()),
        target: target.into(),
    }));
}

pub(super) fn push_unresolved(
    observation: &SemanticCallObservation,
    reason: &str,
    records: &mut Vec<FactRecord>,
) {
    records.push(FactRecord::Unresolved(UnresolvedRecord {
        attributes: Some(json!({
            "compiler_candidate_count": observation.compiler_candidate_count,
            "dispatch": observation.dispatch,
            "form": form_name(observation.form),
            "macro_expanded": observation.macro_expanded,
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

fn form_name(form: SemanticCallForm) -> &'static str {
    match form {
        SemanticCallForm::Direct => "direct",
        SemanticCallForm::Member => "member",
        SemanticCallForm::Constructor => "constructor",
        SemanticCallForm::Operator => "operator",
        SemanticCallForm::Destructor => "destructor",
    }
}

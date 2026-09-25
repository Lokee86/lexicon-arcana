use super::model::{CallObservation, Declaration, MacroCallExpression};
use crate::{EdgeRecord, FactRecord, UnresolvedRecord};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub(super) fn add_reference(
    observation: &CallObservation,
    chain: &[&Declaration],
    depth: usize,
    records: &mut Vec<FactRecord>,
) {
    let macro_declaration = chain.last().unwrap();
    records.push(FactRecord::Edge(EdgeRecord {
        attributes: Some(json!({
            "expansion_depth": depth,
            "role": "macro-expansion",
            "via": chain.iter().map(|value| &value.id).collect::<Vec<_>>(),
        })),
        owner: Some(observation.path.clone()),
        relation: "references".into(),
        source: observation.source_id.clone(),
        span: Some(observation.span.clone()),
        target: macro_declaration.id.clone(),
    }));
}

pub(super) fn call_attributes(
    chain: &[&Declaration],
    original: &MacroCallExpression,
    expanded: &MacroCallExpression,
    bindings: &BTreeMap<String, String>,
    resolution_evidence: &[String],
    call_index: usize,
    alias: bool,
) -> Map<String, Value> {
    let mut evidence = vec![
        "macro-mediation",
        if alias { "macro-alias" } else { "macro-body" },
    ];
    if !bindings.is_empty() {
        evidence.push("argument-substitution");
    }
    evidence.extend(resolution_evidence.iter().map(String::as_str));
    evidence.sort_unstable();
    evidence.dedup();

    let current = chain.last().unwrap();
    let mut attributes = Map::new();
    attributes.insert("evidence".into(), json!(evidence));
    attributes.insert("expansion_depth".into(), json!(chain.len() - 1));
    attributes.insert("indirect".into(), json!("macro"));
    attributes.insert("macro_body_callee".into(), json!(original.callee));
    attributes.insert("macro_call_index".into(), json!(call_index));
    attributes.insert(
        "macro_definition_span".into(),
        json!({
            "path": current.span.path,
            "start_line": current.span.start_line,
            "start_column": current.span.start_column,
            "end_line": current.span.end_line,
            "end_column": current.span.end_column,
        }),
    );
    attributes.insert("substituted_arguments".into(), json!(expanded.arguments));
    attributes.insert("substitutions".into(), json!(bindings));
    attributes.insert(
        "via".into(),
        json!(chain.iter().map(|value| &value.id).collect::<Vec<_>>()),
    );
    attributes
}

pub(super) fn add_call_edge(
    observation: &CallObservation,
    target: &Declaration,
    relation: &str,
    candidate_count: usize,
    mut attributes: Map<String, Value>,
    records: &mut Vec<FactRecord>,
) {
    attributes.insert("candidate_count".into(), json!(candidate_count));
    attributes.insert(
        "resolution".into(),
        json!(if relation == "calls" {
            "definite"
        } else {
            "possible"
        }),
    );
    records.push(FactRecord::Edge(EdgeRecord {
        attributes: Some(Value::Object(attributes)),
        owner: Some(observation.path.clone()),
        relation: relation.into(),
        source: observation.source_id.clone(),
        span: Some(observation.span.clone()),
        target: target.id.clone(),
    }));
}

pub(super) fn add_unresolved(
    observation: &CallObservation,
    reason: &str,
    chain: &[&Declaration],
    call: Option<&MacroCallExpression>,
    bindings: Option<&BTreeMap<String, String>>,
    records: &mut Vec<FactRecord>,
) {
    let mut attributes = Map::new();
    attributes.insert(
        "expansion_depth".into(),
        json!(chain.len().saturating_sub(1)),
    );
    attributes.insert(
        "via".into(),
        json!(chain.iter().map(|value| &value.id).collect::<Vec<_>>()),
    );
    if let Some(call) = call {
        attributes.insert("macro_body_callee".into(), json!(call.callee));
        attributes.insert("substituted_arguments".into(), json!(call.arguments));
        attributes.insert("token_pasting".into(), json!(call.token_pasting));
        attributes.insert("stringification".into(), json!(call.stringification));
        attributes.insert(
            "variadic_substitution".into(),
            json!(call.variadic_substitution),
        );
    }
    if let Some(bindings) = bindings
        && !bindings.is_empty()
    {
        attributes.insert("substitutions".into(), json!(bindings));
    }
    records.push(FactRecord::Unresolved(UnresolvedRecord {
        attributes: Some(Value::Object(attributes)),
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

pub(super) fn add_evidence(attributes: &mut Map<String, Value>, value: &str) {
    let mut evidence = attributes
        .get("evidence")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !evidence.iter().any(|item| item.as_str() == Some(value)) {
        evidence.push(json!(value));
    }
    attributes.insert("evidence".into(), Value::Array(evidence));
}

pub(super) fn render_call(call: &MacroCallExpression) -> String {
    format!("{}({})", call.callee, call.arguments.join(", "))
}

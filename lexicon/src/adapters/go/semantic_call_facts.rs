use crate::{AdapterError, FactRecord, SourceSpan, UnresolvedRecord};

use super::{
    discovery::Inventory,
    observations::{CallForm, CallResolution, Observation, Span},
    semantic_call_targets::{
        TargetEvidence, TargetMaterialization, ensure_call_target, inferred_form,
    },
    semantic_fact_index::FactIndex,
    semantic_facts_support::push_edge,
};

pub(super) fn add(
    observation: &Observation,
    inventory: &Inventory,
    records: &mut Vec<FactRecord>,
    index: &mut FactIndex,
) -> Result<bool, AdapterError> {
    match observation {
        Observation::Symbol {
            semantic_key,
            name,
            namespace,
            container_key,
            owner,
            span,
            generated,
        } => {
            let evidence = TargetEvidence {
                semantic_key,
                name: name.as_deref(),
                namespace: namespace.as_deref(),
                container_key: container_key.as_deref(),
                owner: owner.as_deref(),
                span: span.as_ref(),
                generated: *generated,
            };
            let form = inferred_form(&evidence);
            ensure_call_target(
                evidence,
                form,
                &mut TargetMaterialization {
                    inventory,
                    records,
                    index,
                },
            )?;
            Ok(true)
        }
        Observation::Callsite {
            source_key,
            form,
            resolution,
            expression,
            candidate_namespace,
            candidate_name,
            targets,
            owner,
            span,
        } => {
            let source_id = index.node_id(source_key)?;
            if !index.contains_node(&source_id) {
                return Err(AdapterError::new(format!(
                    "Go semantic call source is not materialized: {source_key:?}"
                )));
            }

            if !targets.is_empty() {
                if !matches!(resolution, CallResolution::Resolved) {
                    return Err(AdapterError::new(
                        "Go semantic callsite has targets without resolved evidence",
                    ));
                }
                let relation = call_relation(*form, targets.len());
                for target in targets {
                    let target_id = ensure_call_target(
                        TargetEvidence::from(target),
                        *form,
                        &mut TargetMaterialization {
                            inventory,
                            records,
                            index,
                        },
                    )?;
                    push_edge(
                        records,
                        index,
                        source_id.clone(),
                        target_id,
                        relation,
                        Some(owner.clone()),
                        Some(source_span(owner, span)),
                    );
                }
                return Ok(true);
            }

            if matches!(resolution, CallResolution::Resolved) {
                return Err(AdapterError::new(
                    "Go semantic callsite is resolved but has no targets",
                ));
            }
            records.push(FactRecord::Unresolved(UnresolvedRecord {
                attributes: None,
                candidate_name: candidate_name.clone(),
                candidate_namespace: candidate_namespace.clone(),
                expression: expression.clone().unwrap_or_default(),
                owner: Some(owner.clone()),
                reason: unresolved_reason(*form, *resolution).into(),
                relation: "calls".into(),
                source: source_id,
                span: Some(source_span(owner, span)),
            }));
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn call_relation(form: CallForm, target_count: usize) -> &'static str {
    if matches!(form, CallForm::Conversion) {
        "converts-to"
    } else if target_count == 1 {
        "calls"
    } else {
        "possible-calls"
    }
}

fn unresolved_reason(form: CallForm, resolution: CallResolution) -> &'static str {
    match resolution {
        CallResolution::Ambiguous => "ambiguous-target",
        CallResolution::Unsupported => "unsupported-form",
        CallResolution::Missing => match form {
            CallForm::Interface | CallForm::Dynamic => "dynamic-target",
            CallForm::Builtin => "builtin-target",
            CallForm::Conversion => "type-conversion",
            CallForm::Direct => "missing-target",
        },
        CallResolution::Resolved => {
            unreachable!("resolved callsites are handled before unresolved")
        }
    }
}

fn source_span(owner: &str, span: &Span) -> SourceSpan {
    SourceSpan {
        path: owner.to_owned(),
        start_line: span.start_line,
        start_column: span.start_column,
        end_line: span.end_line,
        end_column: span.end_column,
    }
}

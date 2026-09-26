use std::collections::BTreeSet;

use crate::{AdapterError, FactRecord, SourceSpan, UnresolvedRecord};

use super::{
    discovery::Inventory,
    identities,
    protocol_records::{CallKind, Record, Span, UnresolvedReason},
    semantic_call_targets::{TargetHints, TargetMaterialization, ensure_call_target},
    semantic_facts_support::{EdgeKey, push_edge},
};

pub(super) fn add(
    record: &Record,
    inventory: &Inventory,
    records: &mut Vec<FactRecord>,
    nodes: &mut BTreeSet<String>,
    edges: &mut BTreeSet<EdgeKey>,
) -> Result<bool, AdapterError> {
    match record {
        Record::Target {
            identity,
            class,
            name,
            namespace,
            container,
        } => {
            ensure_call_target(
                identity,
                *class,
                TargetHints {
                    name: name.as_deref(),
                    namespace: namespace.as_deref(),
                    container: container.as_deref(),
                    owner: None,
                    span: None,
                },
                &mut TargetMaterialization {
                    inventory,
                    records,
                    nodes,
                    edges,
                },
            )?;
            Ok(true)
        }
        Record::Call {
            source,
            target,
            kind,
            class,
            target_name,
            target_namespace,
            target_container,
            target_owner,
            target_span,
            owner,
            span,
        } => {
            let source_id = identities::node_id(source)?;
            if !nodes.contains(&source_id) {
                return Err(AdapterError::new(format!(
                    "Go semantic call source is not materialized: {source:?}"
                )));
            }
            let target_id = ensure_call_target(
                target,
                *class,
                TargetHints {
                    name: target_name.as_deref(),
                    namespace: target_namespace.as_deref(),
                    container: target_container.as_deref(),
                    owner: target_owner.as_deref(),
                    span: target_span.as_ref(),
                },
                &mut TargetMaterialization {
                    inventory,
                    records,
                    nodes,
                    edges,
                },
            )?;
            push_edge(
                records,
                edges,
                source_id,
                target_id,
                call_relation(*kind),
                Some(owner.clone()),
                Some(source_span(owner, span)),
            );
            Ok(true)
        }
        Record::Unresolved {
            source,
            relation,
            expression,
            candidate_namespace,
            candidate_name,
            reason,
            owner,
            span,
            ..
        } => {
            let source_id = identities::node_id(source)?;
            if !nodes.contains(&source_id) {
                return Err(AdapterError::new(format!(
                    "Go semantic unresolved call source is not materialized: {source:?}"
                )));
            }
            records.push(FactRecord::Unresolved(UnresolvedRecord {
                attributes: None,
                candidate_name: candidate_name.clone(),
                candidate_namespace: candidate_namespace.clone(),
                expression: expression.clone(),
                owner: Some(owner.clone()),
                reason: unresolved_reason(*reason).into(),
                relation: relation.clone(),
                source: source_id,
                span: Some(source_span(owner, span)),
            }));
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn call_relation(kind: CallKind) -> &'static str {
    match kind {
        CallKind::Definite => "calls",
        CallKind::Possible => "possible-calls",
        CallKind::Conversion => "converts-to",
    }
}

fn unresolved_reason(reason: UnresolvedReason) -> &'static str {
    match reason {
        UnresolvedReason::MissingTarget => "missing-target",
        UnresolvedReason::AmbiguousTarget => "ambiguous-target",
        UnresolvedReason::UnsupportedForm => "unsupported-form",
        UnresolvedReason::DynamicTarget => "dynamic-target",
        UnresolvedReason::ExternalTarget => "external-target",
        UnresolvedReason::BuiltinTarget => "builtin-target",
        UnresolvedReason::TypeConversion => "type-conversion",
        UnresolvedReason::SelfTarget => "self-target",
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

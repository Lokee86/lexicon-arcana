use serde_json::json;

use super::facts::Facts;
use super::model::{AnalysisContext, CallReference, ParsedFile};
use super::parser::find_calls;
use super::semantic::{OwnerSet, SemanticModel, context_for_position};
use super::syntax::{property_chain, simple_identifier, string_literal};

pub fn process(facts: &mut Facts, model: &SemanticModel, files: &[ParsedFile], file_index: usize) {
    let file = &files[file_index];
    for statement in &file.statements {
        for call in find_calls(statement, &file.path) {
            let context = context_for_position(
                files,
                file_index,
                call.span.start_line,
                call.span.start_column,
            );
            let owner = if context.function_id.is_empty() {
                context.owner_id.clone()
            } else {
                context.function_id.clone()
            };
            emit_resolved(facts, model, files, &context, &owner, &call);
            emit_callbacks(facts, model, files, &context, &owner, &call);
        }
    }
}

fn emit_resolved(
    facts: &mut Facts,
    model: &SemanticModel,
    files: &[ParsedFile],
    context: &AnalysisContext,
    owner: &str,
    call: &CallReference,
) {
    let resolution = model.resolve_call(facts, files, context, call);
    let mut targets = resolution.function_targets;
    targets.extend(resolution.constructor_owners);
    targets.sort();
    targets.dedup();

    match targets.as_slice() {
        [target] => facts.add_edge(owner, target, "calls", Some(call.span.clone()), None),
        values if !values.is_empty() => {
            for target in values {
                facts.add_edge(
                    owner,
                    target,
                    "possible-calls",
                    Some(call.span.clone()),
                    Some(json!({"dispatch": true})),
                );
            }
        }
        _ => facts.add_unresolved(
            owner,
            "calls",
            &call.expression,
            if resolution.reason.is_empty() {
                "dynamic-target"
            } else {
                &resolution.reason
            },
            Some(call.span.clone()),
            (!call.name.is_empty()).then_some(call.callee.clone()),
        ),
    }
}

fn emit_callbacks(
    facts: &mut Facts,
    model: &SemanticModel,
    files: &[ParsedFile],
    context: &AnalysisContext,
    owner: &str,
    call: &CallReference,
) {
    if call.name == "Callable" && call.args.len() >= 2 {
        if let Some(method) = string_literal(&call.args[1]) {
            emit_possible_methods(
                facts,
                model,
                owner,
                model.infer_expression_owners(facts, files, context, &call.args[0]),
                &method,
                &call.span,
                "callable",
            );
        }
        return;
    }

    if matches!(
        call.name.as_str(),
        "call" | "call_deferred" | "rpc" | "rpc_id"
    ) {
        let index = usize::from(call.name == "rpc_id");
        if let Some(argument) = call.args.get(index)
            && let Some(method) = string_literal(argument)
        {
            let owners = if call.receiver.is_empty() {
                OwnerSet::from([context.owner_id.clone()])
            } else {
                model.infer_expression_owners(facts, files, context, &call.receiver)
            };
            emit_possible_methods(
                facts,
                model,
                owner,
                owners,
                &method,
                &call.span,
                "dynamic-invocation",
            );
        }
    }

    let Some(index) = callback_argument_index(&call.name) else {
        return;
    };
    let Some(argument) = call.args.get(index) else {
        return;
    };

    let inferred = model.infer_expression_callables(facts, files, context, argument);
    if !inferred.is_empty() {
        emit_possible_targets(
            facts,
            owner,
            inferred.into_iter().collect(),
            &call.span,
            "callback",
        );
        return;
    }

    if let Some(name) = simple_identifier(argument) {
        emit_possible_targets(
            facts,
            owner,
            model.method_targets(facts, &context.owner_id, name, false, false),
            &call.span,
            "callback",
        );
        return;
    }
    if let Some(parts) = property_chain(argument)
        && parts.len() == 2
        && parts[0] == "self"
    {
        emit_possible_targets(
            facts,
            owner,
            model.method_targets(facts, &context.owner_id, &parts[1], false, false),
            &call.span,
            "callback",
        );
    }
}

fn emit_possible_methods(
    facts: &mut Facts,
    model: &SemanticModel,
    source: &str,
    owners: OwnerSet,
    method: &str,
    span: &crate::SourceSpan,
    reason: &str,
) {
    let mut targets = owners
        .into_iter()
        .flat_map(|owner| model.method_targets(facts, &owner, method, false, false))
        .collect::<Vec<_>>();
    targets.sort();
    targets.dedup();
    emit_possible_targets(facts, source, targets, span, reason);
}

fn emit_possible_targets(
    facts: &mut Facts,
    source: &str,
    mut targets: Vec<String>,
    span: &crate::SourceSpan,
    reason: &str,
) {
    targets.sort();
    targets.dedup();
    for target in targets {
        facts.add_edge(
            source,
            &target,
            "possible-calls",
            Some(span.clone()),
            Some(json!({"callback": true, "reason": reason})),
        );
    }
}

fn callback_argument_index(name: &str) -> Option<usize> {
    matches!(
        name,
        "connect" | "map" | "filter" | "any" | "all" | "sort_custom" | "bsearch_custom" | "reduce"
    )
    .then_some(0)
}

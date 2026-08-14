use crate::flow::Analyzer;
use crate::model::{Context, ValueSet};
use std::collections::BTreeMap;

pub(crate) fn analyze(context: &mut Context) {
    for _ in 0..16 {
        let mut returns = BTreeMap::<String, ValueSet>::new();
        let mut parameters = context.propagated_parameters.clone();
        let mut captures = context.propagated_captures.clone();
        for function in context.functions.values() {
            let result = Analyzer::new(context, function).run();
            returns
                .entry(function.id.clone())
                .or_default()
                .merge(&result.return_value);
            for (key, value) in result.parameter_updates {
                parameters.entry(key).or_default().merge(&value);
            }
            for (key, value) in result.capture_updates {
                captures.entry(key).or_default().merge(&value);
            }
        }
        if returns == context.return_values
            && parameters == context.propagated_parameters
            && captures == context.propagated_captures
        {
            break;
        }
        context.return_values = returns;
        context.propagated_parameters = parameters;
        context.propagated_captures = captures;
    }
    let calls: Vec<_> = context
        .functions
        .values()
        .map(|function| {
            let result = Analyzer::new(context, function).run();
            (function.id.clone(), result.calls)
        })
        .collect();
    for (owner, events) in calls {
        for event in events {
            emit_call(context, &owner, event);
        }
    }
}

fn emit_call(context: &mut Context, owner: &str, event: crate::call_model::CallEvent) {
    if event.resolution.targets.is_empty() {
        context.facts.add_unresolved(
            owner,
            "calls",
            &event.expression,
            event.resolution.reason.unwrap_or("dynamic-target"),
            event.span,
        );
        return;
    }
    let relation = if event.resolution.possible || event.resolution.targets.len() > 1 {
        "possible-calls"
    } else {
        "calls"
    };
    for target in event.resolution.targets {
        context
            .facts
            .add_edge(owner, &target, relation, event.span.clone());
    }
}

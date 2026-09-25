use super::{
    call_candidates::{has_callable, resolve},
    indirect_calls::{IndirectCallIndex, resolve_pointer_declarations},
    macro_fact_records::{
        add_call_edge, add_evidence, add_unresolved, call_attributes, render_call,
    },
    macro_facts::expand,
    macro_resolution::resolve_macros,
    macro_substitution::{argument_candidates, substitute_call},
    model::{CallObservation, Declaration, MacroCallExpression},
    resolution::DeclarationIndex,
};
use crate::FactRecord;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_expanded<'a>(
    index: &'a DeclarationIndex<'a>,
    indirect: &IndirectCallIndex<'a>,
    invocation: &CallObservation,
    body_call: &MacroCallExpression,
    bindings: &BTreeMap<String, String>,
    chain: &[&'a Declaration],
    active: &mut BTreeSet<String>,
    depth: usize,
    possible: bool,
    call_index: usize,
    alias: bool,
    records: &mut Vec<FactRecord>,
) {
    let expanded = if alias {
        body_call.clone()
    } else {
        substitute_call(body_call, bindings)
    };
    if expanded.callee.is_empty() {
        add_unresolved(
            invocation,
            "unsupported-macro-expansion",
            chain,
            Some(body_call),
            Some(bindings),
            records,
        );
        return;
    }

    let observation = CallObservation {
        source_id: invocation.source_id.clone(),
        source_scope: invocation.source_scope.clone(),
        path: invocation.path.clone(),
        expression: render_call(&expanded),
        candidate: expanded.callee.clone(),
        arguments: argument_candidates(&expanded.arguments),
        argument_expressions: expanded.arguments.clone(),
        member: false,
        receiver: String::new(),
        receiver_type_id: String::new(),
        span: invocation.span.clone(),
    };

    let nested = resolve_macros(index, &expanded.callee, &invocation.path);
    if !nested.is_empty() {
        let nested_possible = possible || nested.len() != 1;
        for nested_macro in nested {
            let conditional = nested_macro
                .attributes
                .get("conditional")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            expand(
                index,
                indirect,
                &observation,
                nested_macro,
                chain,
                active,
                depth + 1,
                nested_possible || conditional,
                records,
            );
        }
        return;
    }

    let mut resolution = resolve(index, &observation);
    if resolution.candidates.len() > 1 {
        resolution = resolution.prune(observation.argument_expressions.len());
    }
    let attributes = call_attributes(
        chain,
        body_call,
        &expanded,
        bindings,
        &resolution.evidence,
        call_index,
        alias,
    );

    if resolution.candidates.len() == 1 && !possible {
        add_call_edge(
            &observation,
            resolution.candidates[0],
            "calls",
            1,
            attributes,
            records,
        );
        return;
    }
    if !resolution.candidates.is_empty() {
        let candidate_count = resolution.candidates.len();
        for candidate in &resolution.candidates {
            add_call_edge(
                &observation,
                candidate,
                "possible-calls",
                candidate_count,
                attributes.clone(),
                records,
            );
        }
        return;
    }

    let pointers = resolve_pointer_declarations(index, &observation);
    if !pointers.is_empty() {
        let targets = indirect.targets(&pointers);
        if !targets.is_empty() {
            let target_count = targets.len();
            let mut attributes = attributes;
            add_evidence(&mut attributes, "function-pointer");
            attributes.insert(
                "pointer_via".into(),
                json!(pointers.iter().map(|value| &value.id).collect::<Vec<_>>()),
            );
            for target in targets {
                add_call_edge(
                    &observation,
                    target,
                    "possible-calls",
                    target_count,
                    attributes.clone(),
                    records,
                );
            }
            return;
        }
    }

    let reason = if has_callable(index, &observation.candidate) {
        "missing-target"
    } else {
        "external-target"
    };
    add_unresolved(
        &observation,
        reason,
        chain,
        Some(body_call),
        Some(bindings),
        records,
    );
}

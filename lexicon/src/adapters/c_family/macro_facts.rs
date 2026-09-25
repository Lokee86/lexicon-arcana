use super::{
    indirect_calls::IndirectCallIndex,
    macro_expanded_call::resolve_expanded,
    macro_fact_records::{add_reference, add_unresolved},
    macro_resolution::{MAX_MACRO_EXPANSION_DEPTH, resolve_macros},
    macro_substitution::{invocation_bindings, unsupported_substitution},
    model::{CallObservation, Declaration, MacroCallExpression},
    resolution::DeclarationIndex,
};
use crate::FactRecord;
use serde_json::Value;
use std::collections::BTreeSet;

pub fn try_resolve(
    index: &DeclarationIndex<'_>,
    indirect: &IndirectCallIndex<'_>,
    observation: &CallObservation,
    records: &mut Vec<FactRecord>,
) -> bool {
    let macros = resolve_macros(index, &observation.candidate, &observation.path);
    if macros.is_empty() {
        return false;
    }

    let possible = macros.len() != 1;
    for macro_declaration in macros {
        let conditional = macro_declaration
            .attributes
            .get("conditional")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        expand(
            index,
            indirect,
            observation,
            macro_declaration,
            &[],
            &mut BTreeSet::new(),
            0,
            possible || conditional,
            records,
        );
    }
    true
}

#[allow(clippy::too_many_arguments)]
pub(super) fn expand<'a>(
    index: &'a DeclarationIndex<'a>,
    indirect: &IndirectCallIndex<'a>,
    observation: &CallObservation,
    macro_declaration: &'a Declaration,
    chain: &[&'a Declaration],
    active: &mut BTreeSet<String>,
    depth: usize,
    possible: bool,
    records: &mut Vec<FactRecord>,
) {
    let mut chain = chain.to_vec();
    chain.push(macro_declaration);
    add_reference(observation, &chain, depth, records);

    if depth >= MAX_MACRO_EXPANSION_DEPTH {
        add_unresolved(
            observation,
            "macro-expansion-depth",
            &chain,
            None,
            None,
            records,
        );
        return;
    }
    if !active.insert(macro_declaration.id.clone()) {
        add_unresolved(
            observation,
            "macro-expansion-cycle",
            &chain,
            None,
            None,
            records,
        );
        return;
    }
    if unsupported_substitution(macro_declaration) {
        add_unresolved(
            observation,
            "unsupported-macro-expansion",
            &chain,
            None,
            None,
            records,
        );
        active.remove(&macro_declaration.id);
        return;
    }

    let Some(bindings) = invocation_bindings(macro_declaration, &observation.argument_expressions)
    else {
        add_unresolved(
            observation,
            "macro-argument-mismatch",
            &chain,
            None,
            None,
            records,
        );
        active.remove(&macro_declaration.id);
        return;
    };

    let mut calls = macro_declaration.macro_calls.clone();
    let alias = calls.is_empty() && !macro_declaration.macro_target.is_empty();
    if alias {
        calls.push(MacroCallExpression {
            callee: macro_declaration.macro_target.clone(),
            arguments: observation.argument_expressions.clone(),
            token_pasting: false,
            stringification: false,
            variadic_substitution: false,
            unsupported: false,
        });
    }

    for (call_index, body_call) in calls.iter().enumerate() {
        if body_call.unsupported {
            add_unresolved(
                observation,
                "unsupported-macro-expansion",
                &chain,
                Some(body_call),
                Some(&bindings),
                records,
            );
            continue;
        }
        resolve_expanded(
            index,
            indirect,
            observation,
            body_call,
            &bindings,
            &chain,
            active,
            depth,
            possible,
            call_index,
            alias,
            records,
        );
    }

    active.remove(&macro_declaration.id);
}

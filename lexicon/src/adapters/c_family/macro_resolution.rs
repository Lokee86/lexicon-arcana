use super::{
    macro_substitution::{argument_candidates, invocation_bindings, substitute_call},
    model::{CallObservation, Declaration, MacroCallExpression},
    resolution::DeclarationIndex,
};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_MACRO_EXPANSION_DEPTH: usize = 8;

pub fn resolve_macros<'a>(
    index: &'a DeclarationIndex<'a>,
    candidate: &str,
    path: &str,
) -> Vec<&'a Declaration> {
    let name = super::syntax::last_qualified_part(candidate);
    let mut best_rank = usize::MAX;
    let mut result = Vec::new();
    for value in index.by_name.get(&name).into_iter().flatten() {
        let is_macro = value
            .attributes
            .get("macro")
            .and_then(|attribute| attribute.as_bool())
            .unwrap_or(false);
        if !is_macro || (!value.macro_function && value.macro_target.is_empty()) {
            continue;
        }
        let Some(rank) = index.visibility.include_rank(path, &value.path) else {
            continue;
        };
        if rank < best_rank {
            best_rank = rank;
            result.clear();
        }
        if rank == best_rank {
            result.push(*value);
        }
    }
    result.sort_by(|left, right| left.id.cmp(&right.id));
    result
}

pub fn callable_targets<'a>(
    index: &'a DeclarationIndex<'a>,
    macros: &[&'a Declaration],
    observation: &CallObservation,
) -> Vec<&'a Declaration> {
    let mut active = BTreeSet::new();
    let mut result = BTreeMap::<String, &'a Declaration>::new();
    for macro_declaration in macros {
        collect_callable_targets(
            index,
            macro_declaration,
            observation,
            &mut active,
            0,
            &mut result,
        );
    }
    result.into_values().collect()
}

fn collect_callable_targets<'a>(
    index: &'a DeclarationIndex<'a>,
    macro_declaration: &'a Declaration,
    observation: &CallObservation,
    active: &mut BTreeSet<String>,
    depth: usize,
    result: &mut BTreeMap<String, &'a Declaration>,
) {
    if depth >= MAX_MACRO_EXPANSION_DEPTH || !active.insert(macro_declaration.id.clone()) {
        return;
    }

    let Some(bindings) = invocation_bindings(macro_declaration, &observation.argument_expressions)
    else {
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

    for call in calls {
        if call.unsupported {
            continue;
        }
        let expanded = if alias {
            call
        } else {
            substitute_call(&call, &bindings)
        };
        let nested = CallObservation {
            source_id: observation.source_id.clone(),
            source_scope: observation.source_scope.clone(),
            path: observation.path.clone(),
            expression: expanded.callee.clone(),
            candidate: expanded.callee.clone(),
            arguments: argument_candidates(&expanded.arguments),
            argument_expressions: expanded.arguments.clone(),
            member: false,
            receiver: String::new(),
            receiver_type_id: String::new(),
            span: observation.span.clone(),
        };

        let nested_macros = resolve_macros(index, &expanded.callee, &observation.path);
        if !nested_macros.is_empty() {
            for nested_macro in nested_macros {
                collect_callable_targets(index, nested_macro, &nested, active, depth + 1, result);
            }
            continue;
        }

        let mut resolution = super::call_candidates::resolve(index, &nested);
        if resolution.candidates.len() > 1 {
            resolution = resolution.prune(expanded.arguments.len());
        }
        for target in resolution.candidates {
            result.insert(target.id.clone(), target);
        }
    }

    active.remove(&macro_declaration.id);
}

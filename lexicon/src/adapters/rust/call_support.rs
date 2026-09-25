use crate::adapters::rust::call_builtins::node_callable;
use crate::adapters::rust::call_model::CallResolution;
use crate::adapters::rust::model::{Context, FunctionInfo, ValueSet};
use crate::adapters::rust::resolve;
use crate::adapters::rust::syntax::normalized_tokens;
use std::collections::BTreeSet;
use syn::Path;

pub(crate) fn macro_call(
    context: &Context,
    function: &FunctionInfo,
    path: &Path,
) -> CallResolution {
    let text = normalized_tokens(path);
    let targets: BTreeSet<_> = resolve::resolve_qns(context, &text, function)
        .into_iter()
        .filter_map(|qn| context.macros.get(&qn).cloned())
        .collect();
    if !targets.is_empty() {
        return CallResolution {
            possible: targets.len() > 1,
            targets,
            ..CallResolution::default()
        };
    }
    let name = text.split("::").last().unwrap_or_default();
    let reason = if crate::adapters::rust::call_builtins::builtin_macro(name) {
        "builtin-target"
    } else if resolve::is_external_path(context, &text, function) || text.contains("::") {
        "external-target"
    } else {
        "generated-target"
    };
    CallResolution {
        reason: Some(reason),
        ..CallResolution::default()
    }
}

pub(crate) fn function_item(context: &Context, function: &FunctionInfo, text: &str) -> ValueSet {
    let targets = callable_targets(context, function, text);
    if targets.is_empty() {
        return ValueSet::default();
    }
    ValueSet {
        callables: targets,
        dynamic_callable: false,
        ..ValueSet::default()
    }
}

pub(crate) fn propagate_arguments(
    context: &Context,
    targets: &BTreeSet<String>,
    arguments: &[ValueSet],
    output: &mut std::collections::BTreeMap<(String, usize), ValueSet>,
) {
    for target in targets {
        let Some(info) = context.functions.get(target) else {
            continue;
        };
        for (index, value) in arguments.iter().enumerate().take(info.parameters.len()) {
            output
                .entry((target.clone(), index))
                .or_default()
                .merge(value);
        }
    }
}

pub(crate) fn returns_for_targets(context: &Context, targets: &BTreeSet<String>) -> ValueSet {
    let mut result = ValueSet::default();
    for target in targets {
        if let Some(type_qn) = context.constructor_types.get(target)
            && let Some(type_id) = context.types.get(type_qn)
        {
            result.types.insert(type_id.clone());
        }
        if let Some(value) = context.return_values.get(target) {
            result.merge(value);
        }
    }
    result
}

pub(crate) fn callable_targets(
    context: &Context,
    function: &FunctionInfo,
    text: &str,
) -> BTreeSet<String> {
    let qns = resolve::resolve_qns(context, text, function);
    let mut targets = BTreeSet::new();
    for qn in qns {
        if let Some(id) = context.symbols.get(&qn)
            && (context.functions.contains_key(id)
                || context.constructor_types.contains_key(id)
                || node_callable(context, id))
        {
            targets.insert(id.clone());
        }
        if let Some(id) = context.constructors.get(&qn) {
            targets.insert(id.clone());
        }
    }
    targets
}

pub(crate) fn from_callable_values(context: &Context, value: &ValueSet) -> CallResolution {
    let targets = value.callables.clone();
    CallResolution {
        possible: targets.len() > 1 || value.dynamic_callable,
        return_value: returns_for_targets(context, &targets),
        targets,
        ..CallResolution::default()
    }
}

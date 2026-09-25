use crate::adapters::rust::call_builtins::{
    builtin_function, builtin_method, builtin_unknown_method, common_method_return,
    generated_unknown_method,
};
use crate::adapters::rust::call_model::CallResolution;
use crate::adapters::rust::call_support::{callable_targets, from_callable_values};
use crate::adapters::rust::model::{Context, FunctionInfo, ValueSet};
use crate::adapters::rust::resolve;
use crate::adapters::rust::syntax::normalized_tokens;
use std::collections::BTreeSet;
use syn::ExprPath;

pub(crate) use crate::adapters::rust::call_support::{
    function_item, macro_call, propagate_arguments, returns_for_targets as target_returns,
};

pub(crate) fn path_call(
    context: &Context,
    function: &FunctionInfo,
    path: &ExprPath,
    callee_value: &ValueSet,
) -> CallResolution {
    if path.qself.is_some() {
        return qself_call(context, function, path);
    }
    if !callee_value.callables.is_empty() {
        return from_callable_values(context, callee_value);
    }
    if callee_value.dynamic_callable {
        return CallResolution {
            reason: Some("dynamic-target"),
            ..CallResolution::default()
        };
    }
    if callee_value.builtin || callee_value.external {
        return CallResolution {
            reason: Some(if callee_value.builtin {
                "builtin-target"
            } else {
                "external-target"
            }),
            return_value: callee_value.clone(),
            ..CallResolution::default()
        };
    }
    let text = normalized_tokens(&path.path);
    let targets = callable_targets(context, function, &text);
    if !targets.is_empty() {
        let return_value = target_returns(context, &targets);
        return CallResolution {
            possible: targets.len() > 1,
            targets,
            return_value,
            ..CallResolution::default()
        };
    }
    if let Some((receiver_path, method)) = split_last(&text) {
        let receiver = resolve::value_from_type(context, receiver_path, function);
        let mut associated = method_targets(context, function, &receiver, method, false);
        if associated.targets.is_empty() {
            associated.reason = if receiver.builtin {
                Some("builtin-target")
            } else if receiver.external {
                Some("external-target")
            } else if !receiver.types.is_empty() && builtin_method(method) {
                Some("generated-target")
            } else if !receiver.types.is_empty() {
                Some("missing-target")
            } else {
                None
            };
            if associated.reason.is_some() {
                associated.return_value = common_method_return(&receiver, method);
            }
        }
        if !associated.targets.is_empty() || associated.reason.is_some() {
            return associated;
        }
    }
    let external_path = resolve::is_external_path(context, &text, function)
        || (text.contains("::")
            && text
                .split("::")
                .next()
                .is_some_and(|root| !matches!(root, "crate" | "self" | "super" | "Self")));
    let builtin_path =
        builtin_function(&text) || resolve::is_builtin_path(context, &text, function);
    let reason = if builtin_path {
        "builtin-target"
    } else if external_path {
        "external-target"
    } else {
        "missing-target"
    };
    CallResolution {
        reason: Some(reason),
        return_value: ValueSet {
            builtin: builtin_path,
            external: external_path,
            ..ValueSet::default()
        },
        ..CallResolution::default()
    }
}

pub(crate) fn method_call(
    context: &Context,
    function: &FunctionInfo,
    receiver: &ValueSet,
    name: &str,
) -> CallResolution {
    let mut resolution = method_targets(context, function, receiver, name, true);
    if resolution.targets.is_empty() {
        resolution.reason = Some(
            if receiver.builtin || (!receiver.types.is_empty() && builtin_method(name)) {
                "builtin-target"
            } else if receiver.external {
                "external-target"
            } else if builtin_unknown_method(name) {
                "builtin-target"
            } else if generated_unknown_method(name) {
                "generated-target"
            } else {
                "dynamic-target"
            },
        );
        resolution.return_value = common_method_return(receiver, name);
    }
    resolution
}

fn qself_call(context: &Context, function: &FunctionInfo, path: &ExprPath) -> CallResolution {
    let Some(qself) = &path.qself else {
        return CallResolution::default();
    };
    let self_text = crate::adapters::rust::syntax::type_tokens(&qself.ty);
    let method = path
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
        .unwrap_or_default();
    let mut receiver = resolve::value_from_type(context, &self_text, function);
    if qself.position > 0 {
        let trait_text = path
            .path
            .segments
            .iter()
            .take(qself.position)
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");
        receiver
            .traits
            .extend(resolve::resolve_trait_ids(context, &trait_text, function));
    }
    method_targets(context, function, &receiver, &method, false)
}

fn method_targets(
    context: &Context,
    function: &FunctionInfo,
    receiver: &ValueSet,
    name: &str,
    receiver_syntax: bool,
) -> CallResolution {
    let mut targets = BTreeSet::new();
    for type_id in &receiver.types {
        if let Some(values) = context.method_index.get(&(type_id.clone(), name.into())) {
            targets.extend(values.iter().cloned());
        }
        if let Some(traits) = context.type_traits.get(type_id) {
            for trait_id in traits {
                if let Some(values) = context
                    .trait_method_index
                    .get(&(trait_id.clone(), name.into()))
                {
                    targets.extend(
                        values
                            .iter()
                            .filter(|target| !context.trait_method_ids.contains(*target))
                            .cloned(),
                    );
                }
            }
        }
    }
    for trait_id in &receiver.traits {
        if let Some(values) = context
            .trait_method_index
            .get(&(trait_id.clone(), name.into()))
        {
            targets.extend(
                values
                    .iter()
                    .filter(|target| !context.trait_method_ids.contains(*target))
                    .cloned(),
            );
        }
    }
    if targets.is_empty() && !receiver_syntax {
        let trait_ids = resolve::resolve_trait_ids(
            context,
            &normalized_tokens(
                &syn::parse_str::<syn::Path>(name).unwrap_or_else(|_| syn::parse_quote!(Unknown)),
            ),
            function,
        );
        for trait_id in trait_ids {
            if let Some(values) = context.trait_method_index.get(&(trait_id, name.into())) {
                targets.extend(
                    values
                        .iter()
                        .filter(|target| !context.trait_method_ids.contains(*target))
                        .cloned(),
                );
            }
        }
    }
    let possible = targets.len() > 1
        || !receiver.traits.is_empty()
        || receiver.dynamic_callable
        || receiver.unknown;
    let return_value = target_returns(context, &targets);
    CallResolution {
        targets,
        possible,
        return_value,
        ..CallResolution::default()
    }
}

fn split_last(text: &str) -> Option<(&str, &str)> {
    text.rsplit_once("::")
}

pub(crate) fn returns_for_targets(
    context: &Context,
    targets: &std::collections::BTreeSet<String>,
) -> ValueSet {
    target_returns(context, targets)
}

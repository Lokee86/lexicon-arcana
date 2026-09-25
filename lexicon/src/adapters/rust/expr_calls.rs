use crate::adapters::rust::call_model::CallEvent;
use crate::adapters::rust::call_resolution;
use crate::adapters::rust::expr_callbacks::{merge_callback_return, propagate_known_callback};
use crate::adapters::rust::flow::Analyzer;
use crate::adapters::rust::model::ValueSet;
use crate::adapters::rust::paths::{span_start, span_value};
use crate::adapters::rust::resolve;
use crate::adapters::rust::syntax::normalized_tokens;
use quote::ToTokens;
use syn::spanned::Spanned;

pub(crate) fn path_value(analyzer: &Analyzer<'_>, value: &syn::ExprPath) -> ValueSet {
    if value.qself.is_none() && value.path.segments.len() == 1 {
        let name = value.path.segments[0].ident.to_string();
        if let Some(found) = analyzer.env.get(&name) {
            let mut found = found.clone();
            if found.callables.is_empty()
                && found.types.is_empty()
                && found.traits.is_empty()
                && found.unknown
            {
                found.dynamic_callable = true;
            }
            return found;
        }
    }
    let text = normalized_tokens(&value.path);
    if text == "None" {
        return ValueSet {
            builtin: true,
            ..ValueSet::default()
        };
    }
    let callable = call_resolution::function_item(analyzer.context, analyzer.function, &text);
    if !callable.callables.is_empty() {
        return callable;
    }
    let mut result = ValueSet::default();
    let resolved_qns = resolve::resolve_qns(analyzer.context, &text, analyzer.function);
    let mut found_value = false;
    for qn in &resolved_qns {
        if let Some(value_type) = analyzer.context.value_types.get(qn) {
            result.merge(&resolve::value_from_type(
                analyzer.context,
                value_type,
                analyzer.function,
            ));
            found_value = true;
        }
    }
    if found_value {
        return result;
    }
    result.types.extend(resolve::resolve_type_ids(
        analyzer.context,
        &text,
        analyzer.function,
    ));
    if result.types.is_empty() {
        let qns = resolve::resolve_qns(analyzer.context, &text, analyzer.function);
        for qn in qns {
            if let Some(constructor) = analyzer.context.constructors.get(&qn)
                && let Some(type_qn) = analyzer.context.constructor_types.get(constructor)
                && let Some(type_id) = analyzer.context.types.get(type_qn)
            {
                result.types.insert(type_id.clone());
            }
        }
    }
    if result.types.is_empty() {
        result.builtin = resolve::is_builtin_path(analyzer.context, &text, analyzer.function);
        result.external = !result.builtin
            && resolve::is_external_path(analyzer.context, &text, analyzer.function);
    }
    result.unknown = result.types.is_empty() && !result.builtin && !result.external;
    result
}

pub(crate) fn call(analyzer: &mut Analyzer<'_>, value: &syn::ExprCall) -> ValueSet {
    let callee = analyzer.eval_expr(&value.func);
    let arguments: Vec<_> = value
        .args
        .iter()
        .map(|arg| analyzer.eval_expr(arg))
        .collect();
    let mut resolution = match value.func.as_ref() {
        syn::Expr::Path(path) => {
            call_resolution::path_call(analyzer.context, analyzer.function, path, &callee)
        }
        _ if !callee.callables.is_empty() => crate::adapters::rust::call_model::CallResolution {
            possible: callee.callables.len() > 1 || callee.dynamic_callable,
            return_value: call_resolution::returns_for_targets(analyzer.context, &callee.callables),
            targets: callee.callables.clone(),
            ..Default::default()
        },
        _ => crate::adapters::rust::call_model::CallResolution {
            reason: Some("dynamic-target"),
            ..Default::default()
        },
    };
    if let syn::Expr::Path(path) = value.func.as_ref() {
        let name = normalized_tokens(&path.path);
        if matches!(
            name.as_str(),
            "Some"
                | "Ok"
                | "Err"
                | "Box::new"
                | "Rc::new"
                | "Arc::new"
                | "RefCell::new"
                | "Mutex::new"
                | "RwLock::new"
        ) {
            let mut wrapped = ValueSet {
                builtin: true,
                ..ValueSet::default()
            };
            for argument in &arguments {
                wrapped
                    .contained_types
                    .extend(argument.types.iter().cloned());
                wrapped
                    .contained_types
                    .extend(argument.contained_types.iter().cloned());
                wrapped.callables.extend(argument.callables.iter().cloned());
                wrapped.contained_values.push(argument.clone());
            }
            resolution.return_value.merge(&wrapped);
        }
    }
    call_resolution::propagate_arguments(
        analyzer.context,
        &resolution.targets,
        &arguments,
        &mut analyzer.result.parameter_updates,
    );
    record(analyzer, value, resolution.clone());
    resolution.return_value
}

pub(crate) fn method_call(analyzer: &mut Analyzer<'_>, value: &syn::ExprMethodCall) -> ValueSet {
    let receiver = analyzer.eval_expr(&value.receiver);
    let args: Vec<_> = value
        .args
        .iter()
        .map(|arg| analyzer.eval_expr(arg))
        .collect();
    let method = value.method.to_string();
    let mut resolution =
        call_resolution::method_call(analyzer.context, analyzer.function, &receiver, &method);
    propagate_known_callback(analyzer, &method, &receiver, &args);
    merge_callback_return(analyzer, &method, &args, &mut resolution.return_value);
    let mut propagated = Vec::with_capacity(args.len() + 1);
    propagated.push(receiver);
    propagated.extend(args);
    call_resolution::propagate_arguments(
        analyzer.context,
        &resolution.targets,
        &propagated,
        &mut analyzer.result.parameter_updates,
    );
    record(analyzer, value, resolution.clone());
    resolution.return_value
}

pub(crate) fn closure_value(analyzer: &mut Analyzer<'_>, value: &syn::ExprClosure) -> ValueSet {
    let start = span_start(value.span());
    let Some(id) = analyzer
        .context
        .closure_ids
        .get(&(analyzer.function.source_path.clone(), start.0, start.1))
        .cloned()
    else {
        return ValueSet {
            dynamic_callable: true,
            unknown: true,
            ..ValueSet::default()
        };
    };
    let parameter_names: std::collections::BTreeSet<_> = analyzer
        .context
        .functions
        .get(&id)
        .into_iter()
        .flat_map(|function| function.parameters.iter())
        .map(|parameter| parameter.name.as_str())
        .collect();
    for (name, captured) in &analyzer.env {
        if !parameter_names.contains(name.as_str()) {
            analyzer
                .result
                .capture_updates
                .entry((id.clone(), name.clone()))
                .or_default()
                .merge(captured);
        }
    }
    ValueSet::callable(id, false)
}

pub(crate) fn record<T: ToTokens + Spanned>(
    analyzer: &mut Analyzer<'_>,
    expression: &T,
    resolution: crate::adapters::rust::call_model::CallResolution,
) {
    analyzer.result.calls.push(CallEvent {
        expression: expression.to_token_stream().to_string(),
        span: span_value(expression.span(), &analyzer.function.source_path),
        resolution,
    });
}

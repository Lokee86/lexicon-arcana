use crate::adapters::rust::call_resolution;
use crate::adapters::rust::expr_calls::record;
use crate::adapters::rust::flow::Analyzer;
use crate::adapters::rust::model::ValueSet;
use crate::adapters::rust::resolve;
use crate::adapters::rust::syntax::normalized_tokens;

pub(crate) fn structure(analyzer: &mut Analyzer<'_>, value: &syn::ExprStruct) -> ValueSet {
    for field in &value.fields {
        analyzer.eval_expr(&field.expr);
    }
    if let Some(rest) = &value.rest {
        analyzer.eval_expr(rest);
    }
    let mut result = ValueSet::default();
    result.types.extend(resolve::resolve_type_ids(
        analyzer.context,
        &normalized_tokens(&value.path),
        analyzer.function,
    ));
    if result.types.is_empty() {
        let text = normalized_tokens(&value.path);
        result.builtin = resolve::is_builtin_path(analyzer.context, &text, analyzer.function);
        result.external = !result.builtin
            && resolve::is_external_path(analyzer.context, &text, analyzer.function);
    }
    result.unknown = result.types.is_empty() && !result.builtin && !result.external;
    result
}

pub(crate) fn field(analyzer: &mut Analyzer<'_>, value: &syn::ExprField) -> ValueSet {
    let base = analyzer.eval_expr(&value.base);
    let member = match &value.member {
        syn::Member::Named(name) => name.to_string(),
        syn::Member::Unnamed(index) => index.index.to_string(),
    };
    let mut result = ValueSet::default();
    if let syn::Member::Unnamed(index) = &value.member
        && let Some(element) = base.tuple_elements.get(index.index as usize)
    {
        result.merge(element);
    }
    for type_id in &base.types {
        let Some(qn) = analyzer.context.type_qn_by_id.get(type_id) else {
            continue;
        };
        if let Some(field) = analyzer.context.fields.get(&(qn.clone(), member.clone())) {
            result.merge(&resolve::value_from_type(
                analyzer.context,
                &field.type_text,
                analyzer.function,
            ));
        }
    }
    if result.types.is_empty()
        && result.traits.is_empty()
        && result.callables.is_empty()
        && (base.builtin || base.external)
    {
        result.builtin = base.builtin;
        result.external = base.external;
    }
    result.unknown = result.types.is_empty()
        && result.traits.is_empty()
        && result.callables.is_empty()
        && !result.builtin
        && !result.external;
    result
}

pub(crate) fn assignment(analyzer: &mut Analyzer<'_>, value: &syn::ExprAssign) -> ValueSet {
    let right = analyzer.eval_expr(&value.right);
    if let syn::Expr::Path(path) = value.left.as_ref() {
        if path.path.segments.len() == 1 {
            analyzer.assign_name(&path.path.segments[0].ident.to_string(), &right);
        }
    } else {
        analyzer.eval_expr(&value.left);
    }
    right
}

pub(crate) fn conditional(analyzer: &mut Analyzer<'_>, value: &syn::ExprIf) -> ValueSet {
    analyzer.eval_expr(&value.cond);
    let mut result = analyzer.eval_block(&value.then_branch);
    if let Some((_, else_expr)) = &value.else_branch {
        result.merge(&analyzer.eval_expr(else_expr));
    }
    result
}

pub(crate) fn match_expr(analyzer: &mut Analyzer<'_>, value: &syn::ExprMatch) -> ValueSet {
    let input = analyzer.eval_expr(&value.expr);
    let mut result = ValueSet::default();
    for arm in &value.arms {
        analyzer.bind_pattern(&arm.pat, &input);
        if let Some((_, guard)) = &arm.guard {
            analyzer.eval_expr(guard);
        }
        result.merge(&analyzer.eval_expr(&arm.body));
    }
    result
}

pub(crate) fn macro_call(
    analyzer: &mut Analyzer<'_>,
    path: &syn::Path,
    expression: &syn::Expr,
) -> ValueSet {
    let resolution = call_resolution::macro_call(analyzer.context, analyzer.function, path);
    record(analyzer, expression, resolution);
    let name = normalized_tokens(path);
    ValueSet {
        builtin: matches!(
            name.split("::").last().unwrap_or_default(),
            "concat"
                | "env"
                | "format"
                | "format_args"
                | "include_bytes"
                | "include_str"
                | "stringify"
                | "vec"
                | "write"
                | "writeln"
        ),
        ..ValueSet::default()
    }
}

pub(crate) fn contained<'a>(
    analyzer: &mut Analyzer<'_>,
    values: impl Iterator<Item = &'a syn::Expr>,
) -> ValueSet {
    let mut result = ValueSet {
        builtin: true,
        ..ValueSet::default()
    };
    for value in values {
        let item = analyzer.eval_expr(value);
        result.contained_types.extend(item.types.iter().cloned());
        result
            .contained_types
            .extend(item.contained_types.iter().cloned());
        result.callables.extend(item.callables.iter().cloned());
        result.dynamic_callable |= item.dynamic_callable;
        result.contained_values.push(item);
    }
    result
}

pub(crate) fn tuple<'a>(
    analyzer: &mut Analyzer<'_>,
    values: impl Iterator<Item = &'a syn::Expr>,
) -> ValueSet {
    let tuple_elements: Vec<_> = values.map(|value| analyzer.eval_expr(value)).collect();
    let mut result = ValueSet {
        tuple_elements: tuple_elements.clone(),
        contained_values: tuple_elements,
        builtin: true,
        ..ValueSet::default()
    };
    for item in &result.tuple_elements {
        result.contained_types.extend(item.types.iter().cloned());
        result
            .contained_types
            .extend(item.contained_types.iter().cloned());
        result.callables.extend(item.callables.iter().cloned());
        result.dynamic_callable |= item.dynamic_callable;
    }
    result
}

use crate::adapters::rust::model::{Context, FunctionInfo, ValueSet};
use crate::adapters::rust::resolve::{
    clean_path, resolve_qns, resolve_trait_ids, resolve_type_ids,
};
use crate::adapters::rust::syntax::normalized_tokens;
use syn::{GenericArgument, PathArguments, Type, TypeParamBound};

pub(crate) fn value_from_type(context: &Context, text: &str, function: &FunctionInfo) -> ValueSet {
    let Ok(value) = syn::parse_str::<Type>(text) else {
        return ValueSet {
            unknown: true,
            ..ValueSet::default()
        };
    };
    value_from_syn_type(context, &value, function)
}

fn value_from_syn_type(context: &Context, value: &Type, function: &FunctionInfo) -> ValueSet {
    match value {
        Type::Reference(value) => value_from_syn_type(context, &value.elem, function),
        Type::Paren(value) => value_from_syn_type(context, &value.elem, function),
        Type::Group(value) => value_from_syn_type(context, &value.elem, function),
        Type::Slice(value) => contained_value(context, &value.elem, function),
        Type::Array(value) => contained_value(context, &value.elem, function),
        Type::Tuple(value) => {
            let mut result = ValueSet {
                builtin: true,
                ..ValueSet::default()
            };
            for item in &value.elems {
                let inner = value_from_syn_type(context, item, function);
                result.contained_types.extend(inner.types.iter().cloned());
                result
                    .contained_types
                    .extend(inner.contained_types.iter().cloned());
                result.callables.extend(inner.callables.iter().cloned());
                result.dynamic_callable |= inner.dynamic_callable;
                result.external |= inner.external;
                result.tuple_elements.push(inner);
            }
            result
        }
        Type::BareFn(_) => ValueSet {
            dynamic_callable: true,
            unknown: true,
            ..ValueSet::default()
        },
        Type::Ptr(value) => {
            let inner = value_from_syn_type(context, &value.elem, function);
            ValueSet {
                contained_types: inner.types.union(&inner.contained_types).cloned().collect(),
                contained_values: vec![inner],
                builtin: true,
                ..ValueSet::default()
            }
        }
        Type::Never(_) => ValueSet {
            builtin: true,
            ..ValueSet::default()
        },
        Type::TraitObject(value) => value_from_bounds(context, &value.bounds, function),
        Type::ImplTrait(value) => value_from_bounds(context, &value.bounds, function),
        Type::Path(path) => {
            let text = normalized_tokens(&path.path);
            if let Some(bounds) = function.generic_bounds.get(&text) {
                let mut result = ValueSet {
                    dynamic_callable: bounds.iter().any(|b| b.contains("Fn")),
                    ..ValueSet::default()
                };
                for bound in bounds {
                    result
                        .traits
                        .extend(resolve_trait_ids(context, bound, function));
                }
                result.unknown = result.traits.is_empty();
                return result;
            }
            let mut result = ValueSet::default();
            result
                .types
                .extend(resolve_type_ids(context, &text, function));
            for segment in &path.path.segments {
                if let PathArguments::AngleBracketed(arguments) = &segment.arguments {
                    for argument in &arguments.args {
                        if let GenericArgument::Type(inner) = argument {
                            let inner = value_from_syn_type(context, inner, function);
                            result.contained_types.extend(inner.types.iter().cloned());
                            result
                                .contained_types
                                .extend(inner.contained_types.iter().cloned());
                            result.callables.extend(inner.callables.iter().cloned());
                            result.dynamic_callable |= inner.dynamic_callable;
                            result.contained_values.push(inner);
                        }
                    }
                }
            }
            if result.types.is_empty() {
                if let Some(alias_qn) = resolve_qns(context, &text, function)
                    .into_iter()
                    .find(|qn| context.type_aliases.contains_key(qn))
                {
                    return value_from_type(context, &context.type_aliases[&alias_qn], function);
                }
                result.builtin = is_builtin_path(context, &text, function);
                result.external = !result.builtin && is_external_path(context, &text, function);
                result.unknown = !result.builtin && !result.external;
            }
            result
        }
        _ => ValueSet {
            unknown: true,
            ..ValueSet::default()
        },
    }
}

fn contained_value(context: &Context, value: &Type, function: &FunctionInfo) -> ValueSet {
    let inner = value_from_syn_type(context, value, function);
    ValueSet {
        contained_types: inner.types.union(&inner.contained_types).cloned().collect(),
        dynamic_callable: inner.dynamic_callable,
        contained_values: vec![inner],
        builtin: true,
        ..ValueSet::default()
    }
}

fn value_from_bounds(
    context: &Context,
    bounds: &syn::punctuated::Punctuated<TypeParamBound, syn::Token![+]>,
    function: &FunctionInfo,
) -> ValueSet {
    let mut result = ValueSet::default();
    for bound in bounds {
        if let TypeParamBound::Trait(bound) = bound {
            let text = normalized_tokens(&bound.path);
            result
                .traits
                .extend(resolve_trait_ids(context, &text, function));
            result.dynamic_callable |= text.contains("Fn");
        }
    }
    result.unknown = result.traits.is_empty();
    result
}

pub(crate) fn is_builtin_path(context: &Context, raw: &str, function: &FunctionInfo) -> bool {
    let path = clean_path(raw);
    let root = path.split("::").next().unwrap_or_default();
    if matches!(
        root,
        "std"
            | "core"
            | "alloc"
            | "proc_macro"
            | "test"
            | "bool"
            | "char"
            | "str"
            | "String"
            | "Vec"
            | "Option"
            | "Result"
            | "Box"
            | "usize"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "f32"
            | "f64"
    ) {
        return true;
    }
    context
        .imports
        .get(&function.module_qn)
        .is_some_and(|scope| scope.builtin_aliases.contains(root))
}

pub(crate) fn is_external_path(context: &Context, raw: &str, function: &FunctionInfo) -> bool {
    let path = clean_path(raw);
    let root = path.split("::").next().unwrap_or_default();
    if is_builtin_path(context, raw, function) {
        return false;
    }
    if context
        .crates
        .iter()
        .find(|item| item.qn == function.crate_qn)
        .is_some_and(|item| item.external_crates.contains(root))
    {
        return true;
    }
    context
        .imports
        .get(&function.module_qn)
        .is_some_and(|scope| scope.external_aliases.contains(root))
}

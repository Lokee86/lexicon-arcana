use quote::ToTokens;
use std::collections::BTreeMap;
use syn::{FnArg, GenericParam, Pat, ReturnType, Signature, Type, TypeParamBound};

pub(crate) fn normalized_tokens<T: ToTokens>(value: &T) -> String {
    value
        .to_token_stream()
        .to_string()
        .split_whitespace()
        .collect()
}

pub(crate) fn type_tokens<T: ToTokens>(value: &T) -> String {
    value.to_token_stream().to_string()
}

pub(crate) fn pattern_name(pattern: &Pat) -> Option<String> {
    match pattern {
        Pat::Ident(value) => Some(value.ident.to_string()),
        Pat::Type(value) => pattern_name(&value.pat),
        Pat::Reference(value) => pattern_name(&value.pat),
        _ => None,
    }
}

pub(crate) fn signature_parameters(
    signature: &Signature,
) -> Vec<crate::adapters::rust::model::ParameterInfo> {
    signature
        .inputs
        .iter()
        .map(|argument| match argument {
            FnArg::Receiver(_) => crate::adapters::rust::model::ParameterInfo {
                name: "self".into(),
                type_text: Some("Self".into()),
                callable_bound: false,
            },
            FnArg::Typed(value) => {
                let text = type_tokens(&value.ty);
                crate::adapters::rust::model::ParameterInfo {
                    name: pattern_name(&value.pat).unwrap_or_else(|| "_".into()),
                    callable_bound: is_callable_type(&text),
                    type_text: Some(text),
                }
            }
        })
        .collect()
}

pub(crate) fn closure_parameters(
    closure: &syn::ExprClosure,
) -> Vec<crate::adapters::rust::model::ParameterInfo> {
    closure
        .inputs
        .iter()
        .map(|pattern| {
            let type_text = match pattern {
                Pat::Type(value) => Some(type_tokens(&value.ty)),
                _ => None,
            };
            crate::adapters::rust::model::ParameterInfo {
                name: pattern_name(pattern).unwrap_or_else(|| "_".into()),
                callable_bound: type_text.as_deref().is_some_and(is_callable_type),
                type_text,
            }
        })
        .collect()
}

pub(crate) fn return_type(output: &ReturnType) -> Option<String> {
    match output {
        ReturnType::Default => None,
        ReturnType::Type(_, value) => Some(type_tokens(value)),
    }
}

pub(crate) fn generic_bounds(signature: &Signature) -> BTreeMap<String, Vec<String>> {
    let mut result = BTreeMap::new();
    for parameter in &signature.generics.params {
        if let GenericParam::Type(value) = parameter {
            let bounds = value
                .bounds
                .iter()
                .filter_map(bound_path)
                .collect::<Vec<_>>();
            if !bounds.is_empty() {
                result.insert(value.ident.to_string(), bounds);
            }
        }
    }
    if let Some(where_clause) = &signature.generics.where_clause {
        for predicate in &where_clause.predicates {
            if let syn::WherePredicate::Type(value) = predicate {
                let name = normalized_tokens(&value.bounded_ty);
                let bounds = value
                    .bounds
                    .iter()
                    .filter_map(bound_path)
                    .collect::<Vec<_>>();
                if !bounds.is_empty() {
                    result.entry(name).or_default().extend(bounds);
                }
            }
        }
    }
    result
}

fn bound_path(bound: &TypeParamBound) -> Option<String> {
    match bound {
        TypeParamBound::Trait(value) => Some(normalized_tokens(&value.path)),
        _ => None,
    }
}

pub(crate) fn is_callable_type(text: &str) -> bool {
    text.starts_with("fn(")
        || text.contains("Fn(")
        || text.contains("FnMut(")
        || text.contains("FnOnce(")
}

pub(crate) fn self_type_name(value: &Type) -> String {
    match value {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_else(|| "impl".into()),
        Type::Reference(value) => self_type_name(&value.elem),
        _ => "impl".into(),
    }
}

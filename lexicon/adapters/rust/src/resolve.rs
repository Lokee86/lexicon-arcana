use crate::model::{Context, FunctionInfo};
pub(crate) use crate::type_resolution::{is_builtin_path, is_external_path, value_from_type};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn resolve_type_ids(
    context: &Context,
    path: &str,
    function: &FunctionInfo,
) -> BTreeSet<String> {
    resolve_qns(context, path, function)
        .into_iter()
        .filter_map(|qn| context.types.get(&qn).cloned())
        .collect()
}

pub(crate) fn resolve_trait_ids(
    context: &Context,
    path: &str,
    function: &FunctionInfo,
) -> BTreeSet<String> {
    resolve_qns(context, path, function)
        .into_iter()
        .filter_map(|qn| context.traits.get(&qn).cloned())
        .collect()
}

pub(crate) fn resolve_qns(
    context: &Context,
    raw: &str,
    function: &FunctionInfo,
) -> BTreeSet<String> {
    let path = clean_path(raw);
    if path.is_empty() {
        return BTreeSet::new();
    }
    if path == "Self" {
        return function
            .self_type
            .as_deref()
            .filter(|value| *value != "Self")
            .map(|value| resolve_qns(context, value, function))
            .unwrap_or_default();
    }
    if let Some(rest) = path.strip_prefix("Self::") {
        let mut result = BTreeSet::new();
        if let Some(self_type) = function
            .self_type
            .as_deref()
            .filter(|value| *value != "Self")
        {
            for qn in resolve_qns(context, self_type, function) {
                result.extend(existing(context, [format!("{qn}::{rest}")]));
            }
        }
        return result;
    }
    if let Some(rest) = path.strip_prefix("crate::") {
        return existing(context, [format!("{}::{rest}", function.crate_qn)]);
    }
    if let Some(rest) = path.strip_prefix("self::") {
        return existing(context, [format!("{}::{rest}", function.module_qn)]);
    }
    if path.starts_with("super::") {
        let mut base = function.module_qn.as_str();
        let mut rest = path.as_str();
        while let Some(next) = rest.strip_prefix("super::") {
            base = base
                .rsplit_once("::")
                .map(|(value, _)| value)
                .unwrap_or(&function.crate_qn);
            rest = next;
        }
        return existing(context, [format!("{base}::{rest}")]);
    }
    let (first, suffix) = path
        .split_once("::")
        .map(|(a, b)| (a, Some(b)))
        .unwrap_or((&path, None));
    if let Some(scope) = context.imports.get(&function.module_qn) {
        if let Some(targets) = scope.bindings.get(first) {
            let imported = existing(
                context,
                targets.iter().map(|target| {
                    suffix
                        .map(|rest| format!("{target}::{rest}"))
                        .unwrap_or_else(|| target.clone())
                }),
            );
            if !imported.is_empty() {
                return imported;
            }
        }
    }
    let mut base = function.module_qn.clone();
    loop {
        let local = existing(context, [format!("{base}::{path}")]);
        if !local.is_empty() {
            return local;
        }
        if base == function.crate_qn {
            break;
        }
        let Some((parent, _)) = base.rsplit_once("::") else {
            break;
        };
        base = parent.to_string();
    }
    if let Some(scope) = context.imports.get(&function.module_qn) {
        let globbed = existing(
            context,
            scope
                .glob_modules
                .iter()
                .map(|module| format!("{module}::{path}")),
        );
        if !globbed.is_empty() {
            return globbed;
        }
    }
    if !path.contains("::") {
        let suffix = format!("::{path}");
        return context
            .modules
            .keys()
            .chain(context.symbols.keys())
            .chain(context.types.keys())
            .chain(context.traits.keys())
            .chain(context.macros.keys())
            .chain(context.constructors.keys())
            .chain(context.type_aliases.keys())
            .chain(context.value_types.keys())
            .filter(|qn| qn.ends_with(&suffix))
            .cloned()
            .collect();
    }
    BTreeSet::new()
}

fn existing(context: &Context, candidates: impl IntoIterator<Item = String>) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for candidate in candidates {
        if has_qn(context, &candidate) {
            result.insert(candidate.clone());
        }
        result.extend(expand_module_reexport(context, &candidate));
    }
    result
}

fn expand_module_reexport(context: &Context, candidate: &str) -> BTreeSet<String> {
    let mut module = candidate.rsplit_once("::").map(|(module, _)| module);
    while let Some(current) = module {
        if context.modules.contains_key(current) {
            let rest = &candidate[current.len() + 2..];
            let (first, suffix) = rest
                .split_once("::")
                .map(|(a, b)| (a, Some(b)))
                .unwrap_or((rest, None));
            if let Some(targets) = context
                .imports
                .get(current)
                .and_then(|scope| scope.bindings.get(first))
            {
                let expanded: BTreeSet<_> = targets
                    .iter()
                    .filter_map(|target| {
                        let qn = suffix
                            .map(|tail| format!("{target}::{tail}"))
                            .unwrap_or_else(|| target.clone());
                        has_qn(context, &qn).then_some(qn)
                    })
                    .collect();
                if !expanded.is_empty() {
                    return expanded;
                }
            }
        }
        module = current.rsplit_once("::").map(|(parent, _)| parent);
    }
    BTreeSet::new()
}

pub(crate) fn resolve_any_qns(
    context: &Context,
    path: &str,
    module_qn: &str,
    crate_qn: &str,
) -> BTreeSet<String> {
    let function = FunctionInfo {
        id: String::new(),
        qn: String::new(),
        module_qn: module_qn.into(),
        crate_qn: crate_qn.into(),
        source_path: String::new(),
        body: crate::model::FunctionBody::Expr(syn::parse_quote!(())),
        parameters: Vec::new(),
        return_type: None,
        self_type: None,
        trait_path: None,
        generic_bounds: BTreeMap::new(),
    };
    resolve_qns(context, path, &function)
}

pub(crate) fn clean_path(raw: &str) -> String {
    let mut output = String::new();
    let mut depth = 0usize;
    for ch in raw.trim().trim_start_matches("::").chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 && !ch.is_whitespace() => output.push(ch),
            _ => {}
        }
    }
    output
}

fn has_qn(context: &Context, qn: &str) -> bool {
    context.modules.contains_key(qn)
        || context.symbols.contains_key(qn)
        || context.types.contains_key(qn)
        || context.traits.contains_key(qn)
        || context.macros.contains_key(qn)
        || context.constructors.contains_key(qn)
        || context.type_aliases.contains_key(qn)
        || context.value_types.contains_key(qn)
}

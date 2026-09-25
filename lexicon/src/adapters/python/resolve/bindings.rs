use std::collections::BTreeMap;

use super::super::facts::Facts;
use super::super::model::ImportInfo;

pub struct BindingResolver;

impl BindingResolver {
    pub fn new(_facts: &Facts) -> Self {
        Self
    }

    pub fn resolve_reference(
        &mut self,
        facts: &Facts,
        module_name: &str,
        class_qname: Option<&str>,
        reference: Option<&str>,
        scope_id: Option<&str>,
    ) -> (Option<String>, String) {
        resolve_reference(facts, module_name, class_qname, reference, scope_id)
    }

    pub fn resolve_module_name(
        &mut self,
        facts: &Facts,
        requested: &str,
        source: &str,
    ) -> Option<String> {
        resolve_module_name(facts, requested, source)
    }
}

pub fn resolve_imports(facts: &mut Facts) {
    let imports = facts.imports.clone();
    if imports.is_empty() {
        return;
    }
    let mut results = vec![(None, "unresolved".to_owned()); imports.len()];
    let mut changed = true;
    let mut passes = 0;
    while changed && passes <= imports.len() {
        changed = false;
        passes += 1;
        for (index, info) in imports.iter().enumerate() {
            let result = resolve_import(facts, info);
            results[index] = result.clone();
            let Some(binding) = &info.binding else {
                continue;
            };
            let scope_key = (info.owner_id.clone(), binding.clone());
            if facts.scope_bindings.get(&scope_key) != Some(&result) {
                facts.scope_bindings.insert(scope_key, result.clone());
                changed = true;
            }
            if facts.modules.get(&info.module_name) == Some(&info.owner_id) {
                let key = (info.module_name.clone(), binding.clone());
                if facts.module_bindings.get(&key) != Some(&result) {
                    facts.module_bindings.insert(key, result);
                    changed = true;
                }
            }
        }
    }
    for (info, (target, reason)) in imports.iter().zip(results) {
        if let Some(target) = target {
            facts.add_edge(&info.owner_id, &target, "imports", info.span.clone(), None);
        } else {
            facts.add_unresolved(
                &info.owner_id,
                "imports",
                &info.expression,
                &reason,
                info.span.clone(),
                Some(resolve_relative_module(info)),
            );
        }
    }
}

pub fn resolve_reference(
    facts: &Facts,
    module_name: &str,
    class_qname: Option<&str>,
    reference: Option<&str>,
    scope_id: Option<&str>,
) -> (Option<String>, String) {
    let Some(reference) = reference.filter(|value| !value.is_empty()) else {
        return (None, "unsupported-form".into());
    };
    if builtin(reference) {
        return (None, "builtin-target".into());
    }
    let parts = reference.split('.').collect::<Vec<_>>();
    if let Some(class_qname) = class_qname
        && parts.len() == 2
        && matches!(parts[0], "self" | "cls")
    {
        let candidate = format!("{class_qname}.{}", parts[1]);
        if let Some(id) = facts.symbols.get(&candidate) {
            return (Some(id.clone()), String::new());
        }
    }

    let binding = scope_binding(facts, scope_id, parts[0]).or_else(|| {
        facts
            .module_bindings
            .get(&(module_name.to_owned(), parts[0].to_owned()))
    });
    if let Some((Some(id), _)) = binding {
        if let Some(base) = facts.qnames.get(id) {
            let candidate = std::iter::once(base.as_str())
                .chain(parts.iter().skip(1).copied())
                .collect::<Vec<_>>()
                .join(".");
            if let Some(target) = facts
                .symbols
                .get(&candidate)
                .or_else(|| facts.modules.get(&candidate))
            {
                return (Some(target.clone()), String::new());
            }
        }
        if parts.len() == 1 {
            return (Some(id.clone()), String::new());
        }
    }

    let mut candidates = Vec::new();
    if let Some(scope) = scope_id
        && let Some(mut qname) = facts.qnames.get(scope).cloned()
    {
        while qname.starts_with(module_name) {
            candidates.push(format!("{qname}.{reference}"));
            if qname == module_name || !qname.contains('.') {
                break;
            }
            qname = qname
                .rsplit_once('.')
                .map_or(String::new(), |(parent, _)| parent.to_owned());
        }
    }
    if let Some(class_qname) = class_qname
        && parts.len() == 1
    {
        candidates.push(format!("{class_qname}.{reference}"));
    }
    candidates.push(format!("{module_name}.{reference}"));
    candidates.push(reference.to_owned());
    for candidate in candidates {
        if let Some(id) = facts
            .symbols
            .get(&candidate)
            .or_else(|| facts.modules.get(&candidate))
        {
            return (Some(id.clone()), String::new());
        }
    }
    if matches!(binding, Some((_, reason)) if reason == "external-target") {
        return (None, "external-target".into());
    }
    (None, "missing-target".into())
}

pub fn resolve_relative_module(info: &ImportInfo) -> String {
    if info.relative_level == 0 {
        return info.target_module.clone();
    }
    let current = info.module_name.split('.').collect::<Vec<_>>();
    let mut package = if info.is_package {
        current
    } else {
        current[..current.len().saturating_sub(1)].to_vec()
    };
    let remove = info.relative_level.saturating_sub(1) as usize;
    package.truncate(package.len().saturating_sub(remove));
    package.extend(
        info.target_module
            .split('.')
            .filter(|part| !part.is_empty()),
    );
    package.join(".")
}

fn resolve_import(facts: &Facts, info: &ImportInfo) -> (Option<String>, String) {
    if info.star {
        return (None, "unsupported-form".into());
    }
    let requested = resolve_relative_module(info);
    let module = resolve_module_name(facts, &requested, &info.module_name);
    let Some(target_name) = info.target_name.as_deref() else {
        return module
            .and_then(|name| facts.modules.get(&name).cloned())
            .map_or((None, "external-target".into()), |id| {
                (Some(id), String::new())
            });
    };

    let mut targets = BTreeMap::new();
    for candidate in module_matches(facts, &requested) {
        let symbol = format!("{candidate}.{target_name}");
        if let Some(id) = facts
            .symbols
            .get(&symbol)
            .or_else(|| facts.modules.get(&symbol))
        {
            targets.insert(candidate, id.clone());
        } else if let Some((Some(id), _)) = facts
            .module_bindings
            .get(&(candidate.clone(), target_name.to_owned()))
        {
            targets.insert(candidate, id.clone());
        }
    }
    if let Some(selected) = nearest_module(
        &targets.keys().cloned().collect::<Vec<_>>(),
        &info.module_name,
    ) {
        return (targets.get(&selected).cloned(), String::new());
    }
    if module.is_some() {
        return (None, "missing-target".into());
    }

    let raw = if requested.is_empty() {
        target_name.to_owned()
    } else {
        format!("{requested}.{target_name}")
    };
    facts
        .symbols
        .get(&raw)
        .or_else(|| facts.modules.get(&raw))
        .cloned()
        .map_or((None, "external-target".into()), |id| {
            (Some(id), String::new())
        })
}

pub(in crate::adapters::python) fn resolve_module_name(
    facts: &Facts,
    requested: &str,
    source: &str,
) -> Option<String> {
    let matches = module_matches(facts, requested);
    if matches.iter().any(|value| value == requested) {
        Some(requested.to_owned())
    } else {
        nearest_module(&matches, source)
    }
}

fn module_matches(facts: &Facts, requested: &str) -> Vec<String> {
    facts
        .modules
        .keys()
        .filter(|name| *name == requested || name.ends_with(&format!(".{requested}")))
        .cloned()
        .collect()
}

fn nearest_module(matches: &[String], source: &str) -> Option<String> {
    let source = source.split('.').collect::<Vec<_>>();
    let source_package = &source[..source.len().saturating_sub(1)];
    let mut ranked = matches
        .iter()
        .map(|name| {
            let candidate = name.split('.').collect::<Vec<_>>();
            let package = &candidate[..candidate.len().saturating_sub(1)];
            let score = source_package
                .iter()
                .zip(package)
                .take_while(|(a, b)| a == b)
                .count();
            (score, name)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    let best = ranked.first()?;
    (!ranked.get(1).is_some_and(|other| other.0 == best.0)).then(|| best.1.clone())
}

fn scope_binding<'a>(
    facts: &'a Facts,
    scope: Option<&str>,
    name: &str,
) -> Option<&'a (Option<String>, String)> {
    let mut current = scope;
    while let Some(id) = current {
        if let Some(binding) = facts.scope_bindings.get(&(id.to_owned(), name.to_owned())) {
            return Some(binding);
        }
        current = facts.scope_parents.get(id).map(String::as_str);
    }
    None
}

fn builtin(reference: &str) -> bool {
    let name = reference.split('.').next().unwrap_or(reference);
    matches!(
        name,
        "__import__"
            | "abs"
            | "aiter"
            | "all"
            | "anext"
            | "any"
            | "ascii"
            | "bin"
            | "bool"
            | "breakpoint"
            | "bytearray"
            | "bytes"
            | "callable"
            | "chr"
            | "classmethod"
            | "compile"
            | "complex"
            | "delattr"
            | "dict"
            | "dir"
            | "divmod"
            | "enumerate"
            | "eval"
            | "exec"
            | "filter"
            | "float"
            | "format"
            | "frozenset"
            | "getattr"
            | "globals"
            | "hasattr"
            | "hash"
            | "help"
            | "hex"
            | "id"
            | "input"
            | "int"
            | "isinstance"
            | "issubclass"
            | "iter"
            | "len"
            | "list"
            | "locals"
            | "map"
            | "max"
            | "memoryview"
            | "min"
            | "next"
            | "object"
            | "oct"
            | "open"
            | "ord"
            | "pow"
            | "print"
            | "property"
            | "range"
            | "repr"
            | "reversed"
            | "round"
            | "set"
            | "setattr"
            | "slice"
            | "sorted"
            | "staticmethod"
            | "str"
            | "sum"
            | "super"
            | "tuple"
            | "type"
            | "vars"
            | "zip"
            | "BaseException"
            | "Exception"
            | "ArithmeticError"
            | "AssertionError"
            | "AttributeError"
            | "EOFError"
            | "ImportError"
            | "IndexError"
            | "KeyError"
            | "LookupError"
            | "NameError"
            | "NotImplementedError"
            | "OSError"
            | "RuntimeError"
            | "StopIteration"
            | "SystemExit"
            | "TypeError"
            | "ValueError"
    )
}

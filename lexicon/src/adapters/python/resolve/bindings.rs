mod builtins;
mod imports;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use super::super::facts::Facts;

pub use imports::{resolve_imports, resolve_relative_module};

type ReferenceKey = (String, Option<String>, Option<String>, Option<String>);
type Resolution = (Option<String>, String);

pub struct BindingResolver {
    modules_by_suffix: BTreeMap<String, Vec<String>>,
    module_resolution_cache: BTreeMap<(String, String), Option<String>>,
    reference_cache: BTreeMap<ReferenceKey, Resolution>,
}

impl BindingResolver {
    pub fn new(facts: &Facts) -> Self {
        let mut modules_by_suffix: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for name in facts.modules.keys() {
            let parts = name.split('.').collect::<Vec<_>>();
            for index in 0..parts.len() {
                let suffix = parts[index..].join(".");
                modules_by_suffix
                    .entry(suffix)
                    .or_default()
                    .push(name.clone());
            }
        }
        Self {
            modules_by_suffix,
            module_resolution_cache: BTreeMap::new(),
            reference_cache: BTreeMap::new(),
        }
    }

    pub fn resolve_reference(
        &mut self,
        facts: &Facts,
        module_name: &str,
        class_qname: Option<&str>,
        reference: Option<&str>,
        scope_id: Option<&str>,
    ) -> Resolution {
        let key = (
            module_name.to_owned(),
            class_qname.map(str::to_owned),
            reference.map(str::to_owned),
            scope_id.map(str::to_owned),
        );
        if let Some(value) = self.reference_cache.get(&key) {
            return value.clone();
        }
        let result =
            resolve_reference_uncached(facts, module_name, class_qname, reference, scope_id);
        self.reference_cache.insert(key, result.clone());
        result
    }

    pub fn resolve_module_name(
        &mut self,
        _facts: &Facts,
        requested: &str,
        source: &str,
    ) -> Option<String> {
        let key = (requested.to_owned(), source.to_owned());
        if let Some(value) = self.module_resolution_cache.get(&key) {
            return value.clone();
        }
        let matches = self.module_matches(requested);
        let result = if matches.iter().any(|value| value == requested) {
            Some(requested.to_owned())
        } else {
            self.nearest_module(matches, source)
        };
        self.module_resolution_cache.insert(key, result.clone());
        result
    }

    pub(super) fn module_matches(&self, requested: &str) -> &[String] {
        self.modules_by_suffix
            .get(requested)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(super) fn nearest_module(&self, matches: &[String], source: &str) -> Option<String> {
        nearest_module(matches, source)
    }
}

fn resolve_reference_uncached(
    facts: &Facts,
    module_name: &str,
    class_qname: Option<&str>,
    reference: Option<&str>,
    scope_id: Option<&str>,
) -> Resolution {
    let Some(reference) = reference.filter(|value| !value.is_empty()) else {
        return (None, "unsupported-form".into());
    };
    if builtins::contains(reference) {
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

fn nearest_module(matches: &[String], source: &str) -> Option<String> {
    if matches.is_empty() {
        return None;
    }
    let source = source.split('.').collect::<Vec<_>>();
    let source_package = &source[..source.len().saturating_sub(1)];
    let mut best: Option<(usize, &String)> = None;
    let mut tied = false;

    for name in matches {
        let candidate = name.split('.').collect::<Vec<_>>();
        let package = &candidate[..candidate.len().saturating_sub(1)];
        let score = source_package
            .iter()
            .zip(package)
            .take_while(|(left, right)| left == right)
            .count();
        match best {
            None => {
                best = Some((score, name));
                tied = false;
            }
            Some((best_score, _)) if score > best_score => {
                best = Some((score, name));
                tied = false;
            }
            Some((best_score, best_name)) if score == best_score => {
                if name < best_name {
                    best = Some((score, name));
                }
                tied = true;
            }
            _ => {}
        }
    }

    (!tied)
        .then(|| best.map(|(_, name)| name.clone()))
        .flatten()
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

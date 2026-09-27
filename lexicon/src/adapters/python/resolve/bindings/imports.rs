use std::collections::BTreeMap;

use super::super::super::facts::Facts;
use super::super::super::model::ImportInfo;
use super::BindingResolver;

pub fn resolve_imports(facts: &mut Facts) {
    let imports = std::mem::take(&mut facts.imports);
    if imports.is_empty() {
        return;
    }

    let mut resolver = BindingResolver::new(facts);
    let mut results = vec![(None, "unresolved".to_owned()); imports.len()];
    let mut dependents: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();

    for (index, info) in imports.iter().enumerate() {
        let Some(target_name) = info.target_name.as_deref() else {
            continue;
        };
        let requested = resolve_relative_module(info);
        for candidate in resolver.module_matches(&requested) {
            dependents
                .entry((candidate.clone(), target_name.to_owned()))
                .or_default()
                .push(index);
        }
    }

    let mut queue = (0..imports.len()).collect::<Vec<_>>();
    let mut queued = vec![true; imports.len()];
    let mut cursor = 0;
    while cursor < queue.len() {
        let index = queue[cursor];
        cursor += 1;
        queued[index] = false;

        let info = &imports[index];
        let result = resolve_import(&mut resolver, facts, info);
        results[index] = result.clone();

        let Some(binding) = info.binding.as_deref() else {
            continue;
        };
        let scope_key = (info.owner_id.clone(), binding.to_owned());
        if facts.scope_bindings.get(&scope_key) != Some(&result) {
            facts.scope_bindings.insert(scope_key, result.clone());
        }

        if facts.modules.get(&info.module_name) != Some(&info.owner_id) {
            continue;
        }
        let module_key = (info.module_name.clone(), binding.to_owned());
        if facts.module_bindings.get(&module_key) == Some(&result) {
            continue;
        }
        facts.module_bindings.insert(module_key.clone(), result);

        for &dependent in dependents.get(&module_key).into_iter().flatten() {
            if !queued[dependent] {
                queue.push(dependent);
                queued[dependent] = true;
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
    facts.imports = imports;
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

fn resolve_import(
    resolver: &mut BindingResolver,
    facts: &Facts,
    info: &ImportInfo,
) -> (Option<String>, String) {
    if info.star {
        return (None, "unsupported-form".into());
    }
    let requested = resolve_relative_module(info);
    let module = resolver.resolve_module_name(facts, &requested, &info.module_name);
    let Some(target_name) = info.target_name.as_deref() else {
        return module
            .and_then(|name| facts.modules.get(&name).cloned())
            .map_or((None, "external-target".into()), |id| {
                (Some(id), String::new())
            });
    };

    let mut targets = BTreeMap::new();
    for candidate in resolver.module_matches(&requested) {
        let symbol = format!("{candidate}.{target_name}");
        if let Some(id) = facts
            .symbols
            .get(&symbol)
            .or_else(|| facts.modules.get(&symbol))
        {
            targets.insert(candidate.clone(), id.clone());
        } else if let Some((Some(id), _)) = facts
            .module_bindings
            .get(&(candidate.clone(), target_name.to_owned()))
        {
            targets.insert(candidate.clone(), id.clone());
        }
    }
    if let Some(selected) = resolver.nearest_module(
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

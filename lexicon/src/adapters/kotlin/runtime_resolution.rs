use std::collections::BTreeSet;

use super::model::ParsedFile;
use super::relationships::{explicit_import_names, imported_alias_names, qualify};
use super::runtime::{RuntimeCallable, RuntimeType, runtime_arity_key, runtime_callable_key};
use super::state::AnalysisState;

impl AnalysisState {
    pub fn resolve_runtime_types(
        &self,
        file: &ParsedFile,
        lexical_owner: &str,
        name: &str,
    ) -> (Vec<RuntimeType>, String) {
        let resolve_names = |names: Vec<String>| -> Vec<RuntimeType> {
            unique_types(
                names
                    .into_iter()
                    .flat_map(|name| {
                        self.runtime
                            .types_by_qn
                            .get(&name)
                            .into_iter()
                            .flatten()
                            .filter_map(|id| self.runtime.types_by_id.get(id).cloned())
                            .collect::<Vec<_>>()
                    })
                    .collect(),
            )
        };
        let classify = |targets: Vec<RuntimeType>| {
            let reason = match targets.len() {
                0 => "external-target",
                1 => "",
                _ => "ambiguous-target",
            };
            (targets, reason.into())
        };

        if name.is_empty() {
            return (Vec::new(), "unsupported-form".into());
        }
        if name.contains('.') {
            let targets = resolve_names(vec![name.into()]);
            if !targets.is_empty() {
                return classify(targets);
            }
        }
        let (aliases, bound) = imported_alias_names(file, name);
        if bound {
            return classify(resolve_names(aliases));
        }
        let package = runtime_package(file);
        let lexical = self.resolve_runtime_lexical(lexical_owner, &package, name);
        if !lexical.is_empty() {
            return classify(lexical);
        }
        let (imports, bound) = explicit_import_names(file, name);
        if bound {
            return classify(resolve_names(imports));
        }
        let local = resolve_names(vec![qualify(&package, name)]);
        if !local.is_empty() {
            return classify(local);
        }
        let wildcard = file
            .imports
            .iter()
            .filter(|imported| imported.wildcard)
            .map(|imported| format!("{}.{}", imported.path.trim_end_matches(".*"), name))
            .collect();
        classify(resolve_names(wildcard))
    }

    fn resolve_runtime_lexical(
        &self,
        mut owner: &str,
        package: &str,
        name: &str,
    ) -> Vec<RuntimeType> {
        while !owner.is_empty() && owner != "<default>" && owner != package {
            if let Some(ids) = self.runtime.types_by_qn.get(&format!("{owner}.{name}")) {
                let targets = unique_types(
                    ids.iter()
                        .filter_map(|id| self.runtime.types_by_id.get(id).cloned())
                        .collect(),
                );
                if !targets.is_empty() {
                    return targets;
                }
            }
            owner = owner.rsplit_once('.').map_or("", |(parent, _)| parent);
        }
        Vec::new()
    }

    pub fn constructors_for(&self, owner: &str, arity: usize) -> Vec<RuntimeCallable> {
        unique_callables(
            self.runtime
                .constructors
                .get(&runtime_arity_key(owner, arity))
                .into_iter()
                .flatten()
                .filter_map(|id| self.runtime.callables.get(id).cloned())
                .collect(),
        )
    }

    pub fn same_owner_callables(
        &self,
        callable: &RuntimeCallable,
        name: &str,
        arity: usize,
    ) -> Vec<RuntimeCallable> {
        if !matches!(callable.owner_kind.as_str(), "type" | "interface") {
            return Vec::new();
        }
        self.ordinary_callables(self.runtime.callables_by_key.get(&runtime_callable_key(
            &callable.owner_qn,
            name,
            arity,
        )))
    }

    pub fn package_callables(
        &self,
        callable: &RuntimeCallable,
        name: &str,
        arity: usize,
    ) -> Vec<RuntimeCallable> {
        self.ordinary_callables(self.runtime.callables_by_key.get(&runtime_callable_key(
            &runtime_package(&callable.file),
            name,
            arity,
        )))
    }

    pub fn qualified_callables(
        &self,
        owner: &RuntimeType,
        name: &str,
        arity: usize,
    ) -> Vec<RuntimeCallable> {
        if matches!(
            owner.form.as_str(),
            "object" | "data_object" | "companion_object"
        ) {
            return self.ordinary_callables(
                self.runtime.callables_by_key.get(&runtime_callable_key(
                    &owner.qualified,
                    name,
                    arity,
                )),
            );
        }
        let mut ids = Vec::new();
        for companion_id in self
            .runtime
            .direct_companions_by_owner
            .get(&owner.qualified)
            .into_iter()
            .flatten()
        {
            let Some(companion) = self.runtime.types_by_id.get(companion_id) else {
                continue;
            };
            ids.extend(
                self.runtime
                    .callables_by_key
                    .get(&runtime_callable_key(&companion.qualified, name, arity))
                    .into_iter()
                    .flatten()
                    .cloned(),
            );
        }
        unique_callables(
            ids.into_iter()
                .filter_map(|id| self.runtime.callables.get(&id).cloned())
                .filter(|callable| callable.declaration.receiver.is_empty())
                .collect(),
        )
    }

    fn ordinary_callables(&self, ids: Option<&Vec<String>>) -> Vec<RuntimeCallable> {
        unique_callables(
            ids.into_iter()
                .flatten()
                .filter_map(|id| self.runtime.callables.get(id).cloned())
                .filter(|callable| callable.declaration.receiver.is_empty())
                .collect(),
        )
    }
}

pub fn runtime_package(file: &ParsedFile) -> String {
    if file.package_name.is_empty() {
        "<default>".into()
    } else {
        file.package_name.clone()
    }
}

pub fn unique_types(targets: Vec<RuntimeType>) -> Vec<RuntimeType> {
    let mut seen = BTreeSet::new();
    targets
        .into_iter()
        .filter(|target| seen.insert(target.id.clone()))
        .collect()
}

pub fn unique_callables(targets: Vec<RuntimeCallable>) -> Vec<RuntimeCallable> {
    let mut seen = BTreeSet::new();
    targets
        .into_iter()
        .filter(|target| seen.insert(target.id.clone()))
        .collect()
}

pub fn classify_callables(
    targets: Vec<RuntimeCallable>,
    empty_reason: &str,
) -> (Vec<RuntimeCallable>, String) {
    let targets = unique_callables(targets);
    let reason = match targets.len() {
        0 => empty_reason,
        1 => "",
        _ => "ambiguous-target",
    };
    (targets, reason.into())
}

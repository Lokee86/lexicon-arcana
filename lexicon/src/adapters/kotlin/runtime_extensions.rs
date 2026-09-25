use std::collections::BTreeSet;

use super::relationships::{explicit_import_names, imported_alias_names, qualify};
use super::runtime::{RuntimeCallable, RuntimeType, callable_accepts_arity};
use super::runtime_receiver::RuntimeDeclaredType;
use super::runtime_resolution::{classify_callables, runtime_package, unique_callables};
use super::runtime_tokens::RuntimeInvocation;
use super::state::AnalysisState;

impl AnalysisState {
    pub fn resolve_extension_invocation(
        &self,
        callable: &RuntimeCallable,
        invocation: &RuntimeInvocation,
    ) -> (Vec<RuntimeCallable>, String, bool) {
        if invocation.fluent || invocation.qualifier.contains('.') {
            return (Vec::new(), "unsupported-form".into(), false);
        }
        let Some(evidence) = self.receiver_evidence(callable, invocation) else {
            return (Vec::new(), "dynamic-target".into(), false);
        };
        if evidence.spelling.is_empty() {
            return (Vec::new(), "dynamic-target".into(), true);
        }
        let (receiver, nullable, mut reason) = self.resolve_declared_type(&evidence);
        let Some(receiver) = receiver else {
            if reason == "external-target" {
                reason = "dynamic-target".into();
            }
            return (Vec::new(), reason, true);
        };
        if self.runtime_type_has_ordinary_member(&receiver, &invocation.name, invocation.arity) {
            return (Vec::new(), "dynamic-target".into(), true);
        }

        let (visible, visible_reason) = self.visible_extensions(callable, &invocation.name);
        let mut targets = Vec::new();
        for target in visible {
            if !callable_accepts_arity(&target.declaration, invocation.arity) {
                continue;
            }
            let evidence = RuntimeDeclaredType {
                file: target.file.clone(),
                lexical_owner: target.owner_qn.clone(),
                spelling: target.declaration.receiver.clone(),
            };
            let (target_type, target_nullable, _) = self.resolve_declared_type(&evidence);
            if target_type.is_some_and(|target_type| target_type.id == receiver.id)
                && target_nullable == nullable
            {
                targets.push(target);
            }
        }
        if targets.is_empty() {
            return (
                Vec::new(),
                if visible_reason.is_empty() {
                    "dynamic-target".into()
                } else {
                    visible_reason
                },
                true,
            );
        }
        let (targets, reason) = classify_callables(targets, "dynamic-target");
        (targets, reason, true)
    }

    fn runtime_type_has_ordinary_member(
        &self,
        receiver: &RuntimeType,
        name: &str,
        arity: usize,
    ) -> bool {
        let mut queue = vec![receiver.clone()];
        let mut seen = BTreeSet::new();
        while let Some(current) = queue.first().cloned() {
            queue.remove(0);
            if !seen.insert(current.id.clone()) {
                continue;
            }
            if self
                .runtime
                .has_ordinary_member(&current.qualified, name, arity)
            {
                return true;
            }
            queue.extend(self.direct_runtime_supertypes(&current.qualified));
        }
        false
    }

    fn visible_extensions(
        &self,
        callable: &RuntimeCallable,
        name: &str,
    ) -> (Vec<RuntimeCallable>, String) {
        let resolve = |names: Vec<String>| -> Vec<RuntimeCallable> {
            unique_callables(
                names
                    .into_iter()
                    .flat_map(|qualified| {
                        self.runtime
                            .extensions_by_qn
                            .get(&qualified)
                            .into_iter()
                            .flatten()
                            .filter_map(|id| self.runtime.callables.get(id).cloned())
                            .collect::<Vec<_>>()
                    })
                    .collect(),
            )
        };

        let (aliases, bound) = imported_alias_names(&callable.file, name);
        if bound {
            let targets = resolve(aliases);
            return if targets.is_empty() {
                (Vec::new(), "external-target".into())
            } else {
                (targets, String::new())
            };
        }

        let package = runtime_package(&callable.file);
        let mut owner = callable.owner_qn.as_str();
        while !owner.is_empty() && owner != "<default>" && owner != package {
            let targets = resolve(vec![format!("{owner}.{name}")]);
            if !targets.is_empty() {
                return (targets, String::new());
            }
            owner = owner.rsplit_once('.').map_or("", |(parent, _)| parent);
        }

        let (imports, bound) = explicit_import_names(&callable.file, name);
        if bound {
            let targets = resolve(imports);
            return if targets.is_empty() {
                (Vec::new(), "external-target".into())
            } else {
                (targets, String::new())
            };
        }

        let targets = resolve(vec![qualify(&package, name)]);
        if !targets.is_empty() {
            return (targets, String::new());
        }

        let wildcard = callable
            .file
            .imports
            .iter()
            .filter(|imported| imported.wildcard)
            .map(|imported| format!("{}.{}", imported.path.trim_end_matches(".*"), name))
            .collect();
        let targets = resolve(wildcard);
        if targets.is_empty() {
            (Vec::new(), "dynamic-target".into())
        } else {
            (targets, String::new())
        }
    }
}

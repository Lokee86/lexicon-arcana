use std::collections::BTreeSet;

use super::runtime::RuntimeCallable;
use super::runtime_resolution::{classify_callables, unique_callables};
use super::runtime_tokens::{RuntimeInvocation, invocations};
use super::runtime_values::shadowed_names;
use super::state::AnalysisState;

impl AnalysisState {
    pub fn emit_runtime_semantics(&mut self) {
        self.emit_overrides();
        let ids = self.runtime.callable_ids();
        for id in ids {
            let Some(callable) = self.runtime.callables.get(&id).cloned() else {
                continue;
            };
            self.emit_callable_calls(&callable);
            self.emit_callable_dataflow(&callable);
        }
    }

    fn emit_callable_calls(&mut self, callable: &RuntimeCallable) {
        let shadowed = shadowed_names(callable);
        let invocations = invocations(
            &callable.file,
            &[
                callable.declaration.delegation.clone(),
                callable.declaration.body.clone(),
            ],
        );
        for invocation in invocations {
            let (targets, reason) = self.resolve_invocation(callable, &invocation, &shadowed);
            match targets.as_slice() {
                [] => self.facts.add_unresolved(
                    &callable.id,
                    "calls",
                    &invocation.expression,
                    &reason,
                    Some(&callable.file.path),
                    Some(invocation.span),
                    None,
                ),
                [target] => self.facts.add_edge(
                    &callable.id,
                    &target.id,
                    "calls",
                    Some(&callable.file.path),
                    Some(invocation.span),
                    None,
                ),
                targets => {
                    for target in targets {
                        self.facts.add_edge(
                            &callable.id,
                            &target.id,
                            "possible-calls",
                            Some(&callable.file.path),
                            Some(invocation.span.clone()),
                            None,
                        );
                    }
                }
            }
        }
    }

    fn resolve_invocation(
        &self,
        callable: &RuntimeCallable,
        invocation: &RuntimeInvocation,
        shadowed: &BTreeSet<String>,
    ) -> (Vec<RuntimeCallable>, String) {
        if invocation.unsupported {
            return (Vec::new(), "unsupported-form".into());
        }
        if invocation.qualifier.is_empty() {
            return self.resolve_unqualified_invocation(callable, invocation, shadowed);
        }
        if invocation.qualifier == "this" {
            return classify_callables(
                self.same_owner_callables(callable, &invocation.name, invocation.arity),
                "external-target",
            );
        }
        if invocation.qualifier == "super" {
            return (Vec::new(), "unsupported-form".into());
        }
        self.resolve_qualified_invocation(callable, invocation, shadowed)
    }

    fn resolve_unqualified_invocation(
        &self,
        callable: &RuntimeCallable,
        invocation: &RuntimeInvocation,
        shadowed: &BTreeSet<String>,
    ) -> (Vec<RuntimeCallable>, String) {
        if invocation.name == "this" && callable.kind == "constructor" {
            return classify_callables(
                self.constructors_for(&callable.owner_qn, invocation.arity),
                "external-target",
            );
        }
        if invocation.name == "super" && callable.kind == "constructor" {
            let mut targets = Vec::new();
            for target in self.direct_runtime_supertypes(&callable.owner_qn) {
                targets.extend(self.constructors_for(&target.qualified, invocation.arity));
            }
            return classify_callables(unique_callables(targets), "external-target");
        }
        if shadowed.contains(&invocation.name)
            || callable
                .parameters
                .get(&invocation.name)
                .is_some_and(|values| !values.is_empty())
        {
            return (Vec::new(), "dynamic-target".into());
        }
        let targets = self.same_owner_callables(callable, &invocation.name, invocation.arity);
        if !targets.is_empty() {
            return classify_callables(targets, "external-target");
        }
        let targets = self.package_callables(callable, &invocation.name, invocation.arity);
        if !targets.is_empty() {
            return classify_callables(targets, "external-target");
        }
        let (types, reason) =
            self.resolve_runtime_types(&callable.file, &callable.owner_qn, &invocation.name);
        if types.len() != 1 {
            return (Vec::new(), reason);
        }
        classify_callables(
            self.constructors_for(&types[0].qualified, invocation.arity),
            "external-target",
        )
    }

    fn resolve_qualified_invocation(
        &self,
        callable: &RuntimeCallable,
        invocation: &RuntimeInvocation,
        shadowed: &BTreeSet<String>,
    ) -> (Vec<RuntimeCallable>, String) {
        let (targets, reason, receiver) = self.resolve_extension_invocation(callable, invocation);
        if receiver {
            return (targets, reason);
        }

        let first = invocation
            .qualifier
            .split('.')
            .next()
            .unwrap_or(&invocation.qualifier);
        if shadowed.contains(first)
            || callable
                .parameters
                .get(first)
                .is_some_and(|values| !values.is_empty())
            || self.has_runtime_value(callable, first)
        {
            return (Vec::new(), "dynamic-target".into());
        }

        let (owners, reason) =
            self.resolve_runtime_types(&callable.file, &callable.owner_qn, &invocation.qualifier);
        if owners.len() != 1 {
            return (Vec::new(), reason);
        }
        let mut targets = self.qualified_callables(&owners[0], &invocation.name, invocation.arity);
        let full_name = format!("{}.{}", invocation.qualifier, invocation.name);
        let (nested, nested_reason) =
            self.resolve_runtime_types(&callable.file, &callable.owner_qn, &full_name);
        if nested.len() == 1 {
            targets.extend(self.constructors_for(&nested[0].qualified, invocation.arity));
        } else if targets.is_empty() && nested_reason == "ambiguous-target" {
            return (Vec::new(), nested_reason);
        }
        classify_callables(unique_callables(targets), "external-target")
    }
}

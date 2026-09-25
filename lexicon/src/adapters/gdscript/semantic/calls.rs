use std::collections::BTreeSet;

use super::bindings::{is_callable_invocation, unique};
use super::{OwnerSet, SemanticModel};
use crate::adapters::gdscript::facts::Facts;
use crate::adapters::gdscript::model::{AnalysisContext, CallReference, ParsedFile};
use crate::adapters::gdscript::parser::is_builtin;
use crate::adapters::gdscript::syntax::simple_identifier;

#[derive(Debug, Default)]
pub struct CallResolution {
    pub function_targets: Vec<String>,
    pub constructor_owners: Vec<String>,
    pub reason: String,
}

impl SemanticModel {
    pub fn resolve_call(
        &self,
        facts: &Facts,
        files: &[ParsedFile],
        context: &AnalysisContext,
        call: &CallReference,
    ) -> CallResolution {
        if !call.receiver.is_empty() && is_callable_invocation(&call.name) {
            let targets = self
                .infer_expression_callables(facts, files, context, &call.receiver)
                .into_iter()
                .collect::<Vec<_>>();
            if !targets.is_empty() {
                return CallResolution {
                    function_targets: targets,
                    ..Default::default()
                };
            }
        }

        if call.receiver.is_empty() {
            if call.name == "super" {
                return self.resolve_super_constructor(facts, context);
            }
            if is_builtin(&call.name) {
                return unresolved("builtin-target", false);
            }
            let targets = self.method_targets(facts, &context.owner_id, &call.name, false, false);
            if !targets.is_empty() {
                return CallResolution {
                    function_targets: targets,
                    ..Default::default()
                };
            }
            if self.has_external_parent(facts, &context.owner_id, &mut BTreeSet::new()) {
                return unresolved("external-target", true);
            }
            return unresolved("dynamic-target", false);
        }

        if let Some(receiver_name) = simple_identifier(&call.receiver) {
            if receiver_name == "self" {
                return self.resolve_methods(
                    facts,
                    vec![context.owner_id.clone()],
                    &call.name,
                    false,
                    true,
                );
            }
            if receiver_name == "super" {
                return self.resolve_methods(
                    facts,
                    facts
                        .parent_by_owner_id
                        .get(&context.owner_id)
                        .cloned()
                        .unwrap_or_default(),
                    &call.name,
                    false,
                    true,
                );
            }

            let source_path = &files[context.file_index].path;
            if let Some(owners) = self.preload_alias_owners(facts, source_path, receiver_name) {
                if owners.is_empty() {
                    return unresolved(
                        self.preload_alias_reason(facts, source_path, receiver_name),
                        true,
                    );
                }
                if owners.len() != 1 {
                    return unresolved("ambiguous-target", true);
                }
                if call.name == "new" {
                    return CallResolution {
                        constructor_owners: owners.into_iter().collect(),
                        ..Default::default()
                    };
                }
                return self.resolve_methods(
                    facts,
                    owners.into_iter().collect(),
                    &call.name,
                    true,
                    true,
                );
            }

            let aliases = self.type_alias_owners(source_path, receiver_name);
            if !aliases.is_empty() {
                if aliases.len() != 1 {
                    return unresolved("ambiguous-target", true);
                }
                if call.name == "new" {
                    return CallResolution {
                        constructor_owners: aliases.into_iter().collect(),
                        ..Default::default()
                    };
                }
                return self.resolve_methods(
                    facts,
                    aliases.into_iter().collect(),
                    &call.name,
                    true,
                    true,
                );
            }

            if self.binding_is_builtin(facts, files, context, receiver_name) {
                return unresolved("builtin-target", true);
            }

            let bindings = self.binding_owners(facts, context, receiver_name);
            if !bindings.is_empty() {
                return self.resolve_methods(
                    facts,
                    self.runtime_receiver_owners(facts, bindings.into_iter().collect()),
                    &call.name,
                    false,
                    true,
                );
            }
            if self.binding_declared(facts, context, receiver_name) {
                return unresolved("dynamic-target", true);
            }

            let (classes, ambiguous) = self.class_owners(facts, source_path, receiver_name);
            if !classes.is_empty() || ambiguous {
                if ambiguous {
                    return unresolved("ambiguous-target", true);
                }
                if call.name == "new" {
                    return CallResolution {
                        constructor_owners: classes.into_iter().collect(),
                        ..Default::default()
                    };
                }
                return self.resolve_methods(
                    facts,
                    classes.into_iter().collect(),
                    &call.name,
                    true,
                    true,
                );
            }

            if let Some(owner) = self.autoload_owner(facts, source_path, receiver_name) {
                return self.resolve_methods(facts, vec![owner], &call.name, false, true);
            }
            if is_builtin(receiver_name) {
                return unresolved("builtin-target", true);
            }
        }

        if call.name == "new" {
            let owners = self.infer_type_reference_owners(facts, files, context, &call.receiver);
            if !owners.is_empty() {
                return CallResolution {
                    constructor_owners: owners.into_iter().collect(),
                    ..Default::default()
                };
            }
        }

        let owners = self.infer_expression_owners(facts, files, context, &call.receiver);
        if !owners.is_empty() {
            return self.resolve_methods(
                facts,
                owners.into_iter().collect(),
                &call.name,
                false,
                true,
            );
        }
        unresolved("dynamic-target", false)
    }

    pub fn autoload_owner(&self, facts: &Facts, source_path: &str, name: &str) -> Option<String> {
        let project_root = facts.project_root_by_file_path.get(
            &crate::adapters::gdscript::facts::normalize_path(source_path),
        )?;
        facts
            .autoload_owner_by_project_name
            .get(project_root)
            .and_then(|values| values.get(name))
            .cloned()
    }

    fn resolve_super_constructor(
        &self,
        facts: &Facts,
        context: &AnalysisContext,
    ) -> CallResolution {
        let parents = facts
            .parent_by_owner_id
            .get(&context.owner_id)
            .cloned()
            .unwrap_or_default();
        if parents.is_empty() {
            return unresolved(
                if facts
                    .external_parent_by_owner_id
                    .contains(&context.owner_id)
                {
                    "external-target"
                } else {
                    "missing-target"
                },
                true,
            );
        }
        let targets = unique(
            parents
                .iter()
                .flat_map(|parent| self.method_targets(facts, parent, "_init", false, false))
                .collect(),
        );
        CallResolution {
            function_targets: targets,
            ..Default::default()
        }
    }

    fn resolve_methods(
        &self,
        facts: &Facts,
        owners: Vec<String>,
        name: &str,
        static_only: bool,
        known: bool,
    ) -> CallResolution {
        let targets = unique(
            owners
                .iter()
                .flat_map(|owner| self.method_targets(facts, owner, name, static_only, false))
                .collect(),
        );
        if !targets.is_empty() {
            return CallResolution {
                function_targets: targets,
                ..Default::default()
            };
        }

        let reason = if known {
            if !static_only
                || owners
                    .iter()
                    .any(|owner| self.has_external_parent(facts, owner, &mut BTreeSet::new()))
            {
                "external-target"
            } else {
                "missing-target"
            }
        } else {
            "dynamic-target"
        };
        unresolved(reason, known)
    }

    pub fn method_targets(
        &self,
        facts: &Facts,
        owner: &str,
        name: &str,
        static_only: bool,
        parents_only: bool,
    ) -> Vec<String> {
        self.method_targets_seen(
            facts,
            owner,
            name,
            static_only,
            parents_only,
            &mut BTreeSet::new(),
        )
    }

    fn method_targets_seen(
        &self,
        facts: &Facts,
        owner: &str,
        name: &str,
        static_only: bool,
        parents_only: bool,
        seen: &mut BTreeSet<String>,
    ) -> Vec<String> {
        if owner.is_empty() || !seen.insert(owner.into()) {
            return Vec::new();
        }
        if !parents_only {
            let methods = if static_only {
                facts
                    .static_method_by_owner_id
                    .get(owner)
                    .and_then(|values| values.get(name))
            } else {
                facts
                    .method_by_owner_id
                    .get(owner)
                    .and_then(|values| values.get(name))
            };
            if let Some(methods) = methods
                && !methods.is_empty()
            {
                return unique(methods.clone());
            }
        }

        unique(
            facts
                .parent_by_owner_id
                .get(owner)
                .into_iter()
                .flatten()
                .flat_map(|parent| {
                    self.method_targets_seen(facts, parent, name, static_only, false, seen)
                })
                .collect(),
        )
    }

    fn runtime_receiver_owners(&self, facts: &Facts, owners: Vec<String>) -> Vec<String> {
        let mut result = unique(owners);
        let mut seen = result.iter().cloned().collect::<BTreeSet<_>>();
        let mut index = 0;
        while index < result.len() {
            let owner = result[index].clone();
            for (candidate, parents) in &facts.parent_by_owner_id {
                if parents.iter().any(|parent| parent == &owner) && seen.insert(candidate.clone()) {
                    result.push(candidate.clone());
                }
            }
            index += 1;
        }
        unique(result)
    }

    pub fn binding_owners(&self, facts: &Facts, context: &AnalysisContext, name: &str) -> OwnerSet {
        if !context.function_id.is_empty()
            && let Some(values) = self
                .locals
                .get(&context.function_id)
                .and_then(|values| values.get(name))
            && !values.is_empty()
        {
            return values.clone();
        }
        self.member_owners(facts, &context.owner_id, name, &mut BTreeSet::new())
    }

    pub fn member_owners(
        &self,
        facts: &Facts,
        owner: &str,
        name: &str,
        seen: &mut BTreeSet<String>,
    ) -> OwnerSet {
        if owner.is_empty() || !seen.insert(owner.into()) {
            return OwnerSet::new();
        }
        if let Some(values) = self.members.get(owner).and_then(|values| values.get(name))
            && !values.is_empty()
        {
            return values.clone();
        }
        facts
            .parent_by_owner_id
            .get(owner)
            .into_iter()
            .flatten()
            .flat_map(|parent| self.member_owners(facts, parent, name, seen))
            .collect()
    }
}

fn unresolved(reason: &str, _known: bool) -> CallResolution {
    CallResolution {
        reason: reason.into(),
        ..Default::default()
    }
}

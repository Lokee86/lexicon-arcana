use std::collections::BTreeSet;

use serde_json::json;

use super::calls::builtin;
use super::facts::Facts;
use super::model::{AnalysisState, CallEvidence, Declaration};

impl AnalysisState {
    pub fn resolve_calls(&mut self, facts: &mut Facts) {
        self.calls.sort_by(|left, right| {
            left.owner_path
                .cmp(&right.owner_path)
                .then_with(|| left.span.start_line.cmp(&right.span.start_line))
                .then_with(|| {
                    left.candidate
                        .to_ascii_lowercase()
                        .cmp(&right.candidate.to_ascii_lowercase())
                })
        });
        for evidence in self.calls.clone() {
            let (mut candidates, dynamic, indexed) = self.call_candidates(&evidence);
            if indexed {
                continue;
            }
            candidates.sort_by(|left, right| left.id.cmp(&right.id));
            match (dynamic, candidates.len()) {
                (true, _) => facts.add_unresolved(
                    &evidence.owner_id,
                    "calls",
                    &evidence.expression,
                    "dynamic-target",
                    Some(&evidence.owner_path),
                    Some(evidence.span),
                    Some(json!({"candidate_name": evidence.candidate})),
                ),
                (false, 1) => facts.add_edge(
                    &evidence.owner_id,
                    &candidates[0].id,
                    "calls",
                    Some(&evidence.owner_path),
                    Some(evidence.span),
                    None,
                ),
                (false, count) if count > 1 => facts.add_unresolved(
                    &evidence.owner_id,
                    "calls",
                    &evidence.expression,
                    "ambiguous-target",
                    Some(&evidence.owner_path),
                    Some(evidence.span),
                    Some(json!({
                        "candidate_count": count,
                        "candidate_name": evidence.candidate
                    })),
                ),
                _ => facts.add_unresolved(
                    &evidence.owner_id,
                    "calls",
                    &evidence.expression,
                    if builtin(&evidence.candidate) {
                        "builtin-target"
                    } else {
                        "external-target"
                    },
                    Some(&evidence.owner_path),
                    Some(evidence.span),
                    Some(json!({"candidate_name": evidence.candidate})),
                ),
            }
        }
    }

    fn call_candidates(&self, evidence: &CallEvidence) -> (Vec<Declaration>, bool, bool) {
        let parts = evidence.candidate.split('.').collect::<Vec<_>>();
        if parts.len() > 1 {
            let method = parts[parts.len() - 1].to_ascii_lowercase();
            let qualifier = parts[parts.len() - 2].to_ascii_lowercase();

            if qualifier == "me"
                && let Some(class_id) = evidence.class_id.as_deref()
            {
                if self
                    .resolve_variable_symbol(
                        &evidence.owner_id,
                        evidence.class_id.as_deref(),
                        &evidence.owner_path,
                        &method,
                    )
                    .is_some()
                {
                    return (Vec::new(), false, true);
                }
                return (
                    self.method_candidates(
                        class_id,
                        &method,
                        &evidence.owner_path,
                        true,
                        &mut BTreeSet::new(),
                    ),
                    false,
                    false,
                );
            }

            let visible_classes = self.visible_declarations(
                &evidence.owner_path,
                self.classes_by_name
                    .get(&qualifier)
                    .cloned()
                    .unwrap_or_default(),
            );
            if !visible_classes.is_empty() {
                return (
                    self.methods_for_classes(
                        &visible_classes,
                        &method,
                        &evidence.owner_path,
                        evidence.class_id.as_deref(),
                    ),
                    false,
                    false,
                );
            }

            if let Some(symbol) = self.resolve_variable_symbol(
                &evidence.owner_id,
                evidence.class_id.as_deref(),
                &evidence.owner_path,
                &qualifier,
            ) {
                if symbol.data_type.is_empty() {
                    return (Vec::new(), true, false);
                }
                let classes = self.visible_declarations(
                    &evidence.owner_path,
                    self.classes_by_name
                        .get(&symbol.data_type)
                        .cloned()
                        .unwrap_or_default(),
                );
                let methods = self.methods_for_classes(
                    &classes,
                    &method,
                    &evidence.owner_path,
                    evidence.class_id.as_deref(),
                );
                return if methods.is_empty() {
                    (Vec::new(), true, false)
                } else {
                    (methods, false, false)
                };
            }
            return (Vec::new(), true, false);
        }

        let name = evidence.candidate.to_ascii_lowercase();
        if self
            .resolve_variable_symbol(
                &evidence.owner_id,
                evidence.class_id.as_deref(),
                &evidence.owner_path,
                &name,
            )
            .is_some()
        {
            return (Vec::new(), false, true);
        }
        if let Some(class_id) = evidence.class_id.as_deref() {
            let methods = self.method_candidates(
                class_id,
                &name,
                &evidence.owner_path,
                true,
                &mut BTreeSet::new(),
            );
            if !methods.is_empty() {
                return (methods, false, false);
            }
        }
        (
            self.visible_declarations(
                &evidence.owner_path,
                self.callables_by_name
                    .get(&name)
                    .cloned()
                    .unwrap_or_default(),
            )
            .into_iter()
            .filter(|declaration| declaration.class_id.is_none())
            .collect(),
            false,
            false,
        )
    }

    fn methods_for_classes(
        &self,
        classes: &[Declaration],
        method: &str,
        owner_path: &str,
        current_class: Option<&str>,
    ) -> Vec<Declaration> {
        classes
            .iter()
            .flat_map(|class| {
                let include_private = current_class.is_some_and(|current| {
                    self.class_is_current_or_base(current, &class.id, &mut BTreeSet::new())
                });
                self.method_candidates(
                    &class.id,
                    method,
                    owner_path,
                    include_private,
                    &mut BTreeSet::new(),
                )
            })
            .collect()
    }

    fn method_candidates(
        &self,
        class_id: &str,
        method: &str,
        owner_path: &str,
        include_private: bool,
        visited: &mut BTreeSet<String>,
    ) -> Vec<Declaration> {
        if class_id.is_empty() || !visited.insert(class_id.into()) {
            return Vec::new();
        }
        if let Some(methods) = self
            .methods_by_class
            .get(class_id)
            .and_then(|values| values.get(method))
        {
            let result = methods
                .iter()
                .filter(|value| include_private || value.owner_path == owner_path || value.public)
                .cloned()
                .collect::<Vec<_>>();
            if !result.is_empty() {
                return result;
            }
        }
        self.bases_by_class
            .get(class_id)
            .into_iter()
            .flatten()
            .flat_map(|base| {
                self.method_candidates(base, method, owner_path, include_private, visited)
            })
            .collect()
    }
}

use std::collections::BTreeSet;

use super::model::TokenKind;
use super::runtime::{RuntimeCallable, runtime_member_key};
use super::runtime_resolution::runtime_package;
use super::runtime_tokens::{matching_delimiter, next_token};
use super::state::AnalysisState;
use super::tokens::identifier_text;

pub fn shadowed_names(callable: &RuntimeCallable) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let tokens = &callable.file.tokens;
    let bounds = &callable.declaration.body;
    for index in bounds.start..bounds.end.min(tokens.len()) {
        if matches!(tokens[index].text.as_str(), "val" | "var") {
            let next = next_token(tokens, index + 1, bounds.end);
            if next >= bounds.end {
                continue;
            }
            if tokens[next].kind == TokenKind::Identifier {
                result.insert(identifier_text(&tokens[next]));
                continue;
            }
            if tokens[next].text == "("
                && let Some(close) = matching_delimiter(tokens, next, bounds.end, "(", ")")
            {
                for token in tokens.iter().take(close).skip(next + 1) {
                    if token.kind == TokenKind::Identifier {
                        result.insert(identifier_text(token));
                    }
                }
            }
        }
        if matches!(tokens[index].text.as_str(), "fun" | "class" | "object") {
            let next = next_token(tokens, index + 1, bounds.end);
            if next < bounds.end && tokens[next].kind == TokenKind::Identifier {
                result.insert(identifier_text(&tokens[next]));
            }
        }
        if tokens[index].kind == TokenKind::Identifier {
            let minus = next_token(tokens, index + 1, bounds.end);
            let arrow = next_token(tokens, minus + 1, bounds.end);
            if minus < bounds.end
                && arrow < bounds.end
                && tokens[minus].text == "-"
                && tokens[arrow].text == ">"
            {
                result.insert(identifier_text(&tokens[index]));
            }
        }
    }
    result
}

impl AnalysisState {
    pub fn has_runtime_value(&self, callable: &RuntimeCallable, name: &str) -> bool {
        if callable
            .parameters
            .get(name)
            .is_some_and(|values| !values.is_empty())
        {
            return true;
        }
        let mut owners = vec![callable.owner_qn.clone()];
        if matches!(callable.owner_kind.as_str(), "type" | "interface") {
            owners.push(runtime_package(&callable.file));
        }
        owners.into_iter().any(|owner| {
            self.runtime
                .properties_by_key
                .get(&runtime_member_key(&owner, name))
                .is_some_and(|values| !values.is_empty())
        })
    }

    pub fn resolve_runtime_value(
        &self,
        callable: &RuntimeCallable,
        qualifier: &str,
        name: &str,
        shadowed: &BTreeSet<String>,
    ) -> String {
        if qualifier.is_empty() {
            if shadowed.contains(name) {
                return String::new();
            }
            if let Some(targets) = callable.parameters.get(name) {
                return if targets.len() == 1 {
                    targets[0].clone()
                } else {
                    String::new()
                };
            }
            if matches!(callable.owner_kind.as_str(), "type" | "interface") {
                let target = self.unique_runtime_property(&callable.owner_qn, name);
                if !target.is_empty() {
                    return target;
                }
            }
            return self.unique_runtime_property(&runtime_package(&callable.file), name);
        }
        if qualifier == "this" {
            return self.unique_runtime_property(&callable.owner_qn, name);
        }
        let first = qualifier.split('.').next().unwrap_or(qualifier);
        if shadowed.contains(first)
            || callable
                .parameters
                .get(first)
                .is_some_and(|values| !values.is_empty())
            || self.has_runtime_value(callable, first)
        {
            return String::new();
        }
        let (owners, _) = self.resolve_runtime_types(&callable.file, &callable.owner_qn, qualifier);
        if owners.len() != 1 {
            return String::new();
        }
        let owner = &owners[0];
        if matches!(
            owner.form.as_str(),
            "object" | "data_object" | "companion_object"
        ) {
            return self.unique_runtime_property(&owner.qualified, name);
        }
        let mut targets = Vec::new();
        for companion_id in self
            .runtime
            .direct_companions_by_owner
            .get(&owner.qualified)
            .into_iter()
            .flatten()
        {
            if let Some(companion) = self.runtime.types_by_id.get(companion_id) {
                let target = self.unique_runtime_property(&companion.qualified, name);
                if !target.is_empty() {
                    targets.push(target);
                }
            }
        }
        targets.sort();
        targets.dedup();
        if targets.len() == 1 {
            targets.remove(0)
        } else {
            String::new()
        }
    }

    fn unique_runtime_property(&self, owner: &str, name: &str) -> String {
        let mut targets = self
            .runtime
            .properties_by_key
            .get(&runtime_member_key(owner, name))
            .into_iter()
            .flatten()
            .filter(|property| {
                property.declaration.receiver.is_empty() && !property.declaration.delegated
            })
            .map(|property| property.id.clone())
            .collect::<Vec<_>>();
        targets.sort();
        targets.dedup();
        if targets.len() == 1 {
            targets.remove(0)
        } else {
            String::new()
        }
    }
}

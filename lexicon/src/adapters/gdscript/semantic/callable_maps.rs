use super::{KeyedCallables, OwnerSet, SemanticModel};
use crate::adapters::gdscript::facts::Facts;
use crate::adapters::gdscript::model::{AnalysisContext, ParsedFile, Token};
use crate::adapters::gdscript::parser::terminal_call;
use crate::adapters::gdscript::syntax::{
    simple_identifier, split_arguments, string_literal, top_level_token, top_level_token_after,
    trim_expression,
};

impl SemanticModel {
    pub fn infer_expression_callable_map(
        &self,
        facts: &Facts,
        files: &[ParsedFile],
        context: &AnalysisContext,
        expression: &[Token],
    ) -> KeyedCallables {
        let expression = trim_expression(expression);
        if expression.is_empty() {
            return KeyedCallables::new();
        }

        if let Some(branch) = top_level_token(expression, "if")
            && branch > 0
            && let Some(otherwise) = top_level_token_after(expression, "else", Some(branch))
        {
            return merge_maps([
                self.infer_expression_callable_map(facts, files, context, &expression[..branch]),
                self.infer_expression_callable_map(
                    facts,
                    files,
                    context,
                    &expression[otherwise + 1..],
                ),
            ]);
        }

        if expression.first().is_some_and(|token| token.text == "{")
            && expression.last().is_some_and(|token| token.text == "}")
        {
            return self.dictionary_literal_callables(
                facts,
                files,
                context,
                &expression[1..expression.len() - 1],
            );
        }

        if let Some(name) = simple_identifier(expression) {
            return self.callable_map_binding(facts, context, name);
        }

        if let Some(call) = terminal_call(expression) {
            let resolution = self.resolve_call(facts, files, context, &call);
            return merge_maps(
                resolution
                    .function_targets
                    .into_iter()
                    .filter_map(|function| self.return_callable_maps.get(&function).cloned()),
            );
        }

        KeyedCallables::new()
    }

    fn dictionary_literal_callables(
        &self,
        facts: &Facts,
        files: &[ParsedFile],
        context: &AnalysisContext,
        entries: &[Token],
    ) -> KeyedCallables {
        let mut result = KeyedCallables::new();
        for entry in split_arguments(entries) {
            let Some(colon) = top_level_token(&entry, ":") else {
                continue;
            };
            if colon == 0 || colon + 1 >= entry.len() {
                continue;
            }
            let Some(key) = string_literal(&entry[..colon]) else {
                continue;
            };
            let targets =
                self.infer_expression_callables(facts, files, context, &entry[colon + 1..]);
            if !targets.is_empty() {
                result.entry(key).or_default().extend(targets);
            }
        }
        result
    }

    fn callable_map_binding(
        &self,
        facts: &Facts,
        context: &AnalysisContext,
        name: &str,
    ) -> KeyedCallables {
        if !context.function_id.is_empty()
            && let Some(values) = self
                .local_callable_maps
                .get(&context.function_id)
                .and_then(|values| values.get(name))
            && !values.is_empty()
        {
            return values.clone();
        }
        self.member_callable_map(facts, &context.owner_id, name, &mut Default::default())
    }

    fn member_callable_map(
        &self,
        facts: &Facts,
        owner: &str,
        name: &str,
        seen: &mut std::collections::BTreeSet<String>,
    ) -> KeyedCallables {
        if owner.is_empty() || !seen.insert(owner.into()) {
            return KeyedCallables::new();
        }
        if let Some(values) = self
            .member_callable_maps
            .get(owner)
            .and_then(|values| values.get(name))
            && !values.is_empty()
        {
            return values.clone();
        }
        merge_maps(
            facts
                .parent_by_owner_id
                .get(owner)
                .into_iter()
                .flatten()
                .map(|parent| self.member_callable_map(facts, parent, name, seen)),
        )
    }

    pub fn callable_map_lookup(
        &self,
        facts: &Facts,
        files: &[ParsedFile],
        context: &AnalysisContext,
        receiver: &[Token],
        key: &str,
    ) -> OwnerSet {
        if key.is_empty() {
            return OwnerSet::new();
        }
        if let Some(name) = simple_identifier(receiver) {
            return self
                .callable_map_binding(facts, context, name)
                .remove(key)
                .unwrap_or_default();
        }
        if let Some(call) = terminal_call(receiver) {
            let resolution = self.resolve_call(facts, files, context, &call);
            return resolution
                .function_targets
                .into_iter()
                .filter_map(|function| {
                    self.return_callable_maps
                        .get(&function)
                        .and_then(|values| values.get(key))
                })
                .flatten()
                .cloned()
                .collect();
        }
        OwnerSet::new()
    }

    pub fn add_member_callable_map(
        &mut self,
        owner: &str,
        name: &str,
        values: KeyedCallables,
    ) -> bool {
        if owner.is_empty() || name.is_empty() || values.is_empty() {
            return false;
        }
        merge_map_into(
            self.member_callable_maps
                .entry(owner.into())
                .or_default()
                .entry(name.into())
                .or_default(),
            values,
        )
    }

    pub fn add_local_callable_map(
        &mut self,
        function: &str,
        name: &str,
        values: KeyedCallables,
    ) -> bool {
        if function.is_empty() || name.is_empty() || values.is_empty() {
            return false;
        }
        merge_map_into(
            self.local_callable_maps
                .entry(function.into())
                .or_default()
                .entry(name.into())
                .or_default(),
            values,
        )
    }

    pub fn add_return_callable_map(&mut self, function: &str, values: KeyedCallables) -> bool {
        if function.is_empty() || values.is_empty() {
            return false;
        }
        merge_map_into(
            self.return_callable_maps
                .entry(function.into())
                .or_default(),
            values,
        )
    }
}

fn merge_maps(values: impl IntoIterator<Item = KeyedCallables>) -> KeyedCallables {
    let mut result = KeyedCallables::new();
    for value in values {
        merge_map_into(&mut result, value);
    }
    result
}

fn merge_map_into(target: &mut KeyedCallables, values: KeyedCallables) -> bool {
    let mut changed = false;
    for (key, targets) in values {
        let entry = target.entry(key).or_default();
        let before = entry.len();
        entry.extend(targets);
        changed |= entry.len() != before;
    }
    changed
}

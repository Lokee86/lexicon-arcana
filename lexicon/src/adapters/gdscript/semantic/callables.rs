use std::collections::BTreeSet;

use super::bindings::{merge_binding, merge_set, union};
use super::{OwnerSet, SemanticModel};
use crate::adapters::gdscript::facts::Facts;
use crate::adapters::gdscript::model::{AnalysisContext, ParsedFile, Token};
use crate::adapters::gdscript::parser::terminal_call;
use crate::adapters::gdscript::syntax::{
    property_chain, simple_identifier, string_literal, top_level_token, top_level_token_after,
    trim_expression,
};

impl SemanticModel {
    pub fn infer_expression_callables(
        &self,
        facts: &Facts,
        files: &[ParsedFile],
        context: &AnalysisContext,
        expression: &[Token],
    ) -> OwnerSet {
        let expression = trim_expression(expression);
        if expression.is_empty() {
            return OwnerSet::new();
        }

        if let Some(target) = lambda_target(&files[context.file_index], expression) {
            return OwnerSet::from([target]);
        }

        if let Some(branch) = top_level_token(expression, "if")
            && branch > 0
            && let Some(otherwise) = top_level_token_after(expression, "else", Some(branch))
        {
            return union([
                self.infer_expression_callables(facts, files, context, &expression[..branch]),
                self.infer_expression_callables(
                    facts,
                    files,
                    context,
                    &expression[otherwise + 1..],
                ),
            ]);
        }

        if let Some(call) = terminal_call(expression) {
            if call.name == "get" && !call.args.is_empty() {
                let mut targets = self.callable_map_lookup(
                    facts,
                    files,
                    context,
                    &call.receiver,
                    string_literal(&call.args[0]).as_deref().unwrap_or(""),
                );
                if let Some(default) = call.args.get(1) {
                    targets.extend(self.infer_expression_callables(facts, files, context, default));
                }
                if !targets.is_empty() {
                    return targets;
                }
            }
            if call.name == "Callable" && call.args.len() >= 2 {
                return self.callable_constructor_targets(
                    facts,
                    files,
                    context,
                    &call.args[0],
                    &call.args[1],
                );
            }
            if matches!(call.name.as_str(), "bind" | "bindv" | "unbind") {
                return self.infer_expression_callables(facts, files, context, &call.receiver);
            }
            let resolution = self.resolve_call(facts, files, context, &call);
            return resolution
                .function_targets
                .into_iter()
                .flat_map(|function| {
                    self.return_callables
                        .get(&function)
                        .cloned()
                        .unwrap_or_default()
                })
                .collect();
        }

        if let Some(name) = simple_identifier(expression) {
            let values = self.callable_binding_targets(facts, context, name);
            if !values.is_empty() {
                return values;
            }
            return self
                .method_targets(facts, &context.owner_id, name, false, false)
                .into_iter()
                .collect();
        }

        if let Some(parts) = property_chain(expression)
            && parts.len() == 2
        {
            if parts[0] == "self" {
                return self
                    .method_targets(facts, &context.owner_id, &parts[1], false, false)
                    .into_iter()
                    .collect();
            }
            return self
                .binding_owners(facts, context, &parts[0])
                .into_iter()
                .flat_map(|owner| self.method_targets(facts, &owner, &parts[1], false, false))
                .collect();
        }

        OwnerSet::new()
    }

    pub fn callable_constructor_targets(
        &self,
        facts: &Facts,
        files: &[ParsedFile],
        context: &AnalysisContext,
        receiver: &[Token],
        method_expression: &[Token],
    ) -> OwnerSet {
        let Some(method) = string_literal(method_expression) else {
            return OwnerSet::new();
        };
        let mut owners = self.infer_expression_owners(facts, files, context, receiver);
        if owners.is_empty() && simple_identifier(receiver) == Some("self") {
            owners.insert(context.owner_id.clone());
        }
        owners
            .into_iter()
            .flat_map(|owner| self.method_targets(facts, &owner, &method, false, false))
            .collect()
    }

    pub fn callable_binding_targets(
        &self,
        facts: &Facts,
        context: &AnalysisContext,
        name: &str,
    ) -> OwnerSet {
        if !context.function_id.is_empty()
            && let Some(values) = self
                .local_callables
                .get(&context.function_id)
                .and_then(|values| values.get(name))
            && !values.is_empty()
        {
            return values.clone();
        }
        self.member_callable_targets(facts, &context.owner_id, name, &mut BTreeSet::new())
    }

    fn member_callable_targets(
        &self,
        facts: &Facts,
        owner: &str,
        name: &str,
        seen: &mut BTreeSet<String>,
    ) -> OwnerSet {
        if owner.is_empty() || !seen.insert(owner.into()) {
            return OwnerSet::new();
        }
        if let Some(values) = self
            .member_callables
            .get(owner)
            .and_then(|values| values.get(name))
            && !values.is_empty()
        {
            return values.clone();
        }
        facts
            .parent_by_owner_id
            .get(owner)
            .into_iter()
            .flatten()
            .flat_map(|parent| self.member_callable_targets(facts, parent, name, seen))
            .collect()
    }

    pub fn add_member_callable(&mut self, owner: &str, name: &str, values: OwnerSet) -> bool {
        if owner.is_empty() || name.is_empty() || values.is_empty() {
            return false;
        }
        merge_binding(
            self.member_callables.entry(owner.into()).or_default(),
            name,
            values,
        )
    }

    pub fn add_local_callable(&mut self, function: &str, name: &str, values: OwnerSet) -> bool {
        if function.is_empty() || name.is_empty() || values.is_empty() {
            return false;
        }
        merge_binding(
            self.local_callables.entry(function.into()).or_default(),
            name,
            values,
        )
    }

    pub fn add_return_callable(&mut self, function: &str, values: OwnerSet) -> bool {
        if function.is_empty() || values.is_empty() {
            return false;
        }
        merge_set(
            self.return_callables.entry(function.into()).or_default(),
            values,
        )
    }
}

fn lambda_target(file: &ParsedFile, expression: &[Token]) -> Option<String> {
    let token = expression.iter().find(|token| token.text == "func")?;
    file.declarations
        .iter()
        .find(|declaration| {
            declaration.keyword == "lambda"
                && declaration.span.start_line == token.line
                && declaration.span.start_column == token.column
        })
        .map(|declaration| declaration.node_id.clone())
}

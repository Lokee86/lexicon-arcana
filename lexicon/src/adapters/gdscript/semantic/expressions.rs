use super::bindings::union;
use super::{OwnerSet, SemanticModel};
use crate::adapters::gdscript::facts::Facts;
use crate::adapters::gdscript::model::{AnalysisContext, ParsedFile, Token};
use crate::adapters::gdscript::parser::{project_resource_path, terminal_call};
use crate::adapters::gdscript::syntax::{
    property_chain, simple_identifier, top_level_token, top_level_token_after, trim_expression,
};

impl SemanticModel {
    pub fn infer_expression_owners(
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

        if let Some(cast) = top_level_token(expression, "as")
            && cast > 0
            && cast + 1 < expression.len()
        {
            return self.type_owners(
                facts,
                &files[context.file_index].path,
                &crate::adapters::gdscript::syntax::join_tokens(&expression[cast + 1..]),
            );
        }

        if let Some(branch) = top_level_token(expression, "if")
            && branch > 0
            && let Some(otherwise) = top_level_token_after(expression, "else", Some(branch))
        {
            return union([
                self.infer_expression_owners(facts, files, context, &expression[..branch]),
                self.infer_expression_owners(facts, files, context, &expression[otherwise + 1..]),
            ]);
        }

        if let Some(owners) = self.static_load_owners(facts, files, context, expression) {
            return owners;
        }

        if let Some(call) = terminal_call(expression) {
            let resolution = self.resolve_call(facts, files, context, &call);
            let mut owners = resolution
                .constructor_owners
                .into_iter()
                .collect::<OwnerSet>();
            for target in resolution.function_targets {
                if let Some(values) = self.returns.get(&target) {
                    owners.extend(values.iter().cloned());
                }
            }
            if !owners.is_empty() {
                return owners;
            }
            if call.name == "duplicate" && !call.receiver.is_empty() {
                return self.infer_expression_owners(facts, files, context, &call.receiver);
            }
            return OwnerSet::new();
        }

        if let Some((base, property)) = terminal_property_access(expression) {
            let owners = self.infer_expression_owners(facts, files, context, base);
            let mut result = OwnerSet::new();
            for owner in owners {
                result.extend(self.member_owners(facts, &owner, property, &mut Default::default()));
                if let Some(nested) = facts
                    .type_by_owner_id
                    .get(&owner)
                    .and_then(|values| values.get(property))
                {
                    result.extend(nested.iter().cloned());
                }
            }
            return result;
        }

        if let Some(name) = simple_identifier(expression) {
            let file = &files[context.file_index];
            if let Some(values) = self.preload_alias_owners(facts, &file.path, name) {
                return values;
            }
            let aliases = self.type_alias_owners(&file.path, name);
            if !aliases.is_empty() {
                return aliases;
            }
            if name == "self" {
                return OwnerSet::from([context.owner_id.clone()]);
            }
            let bindings = self.binding_owners(facts, context, name);
            if !bindings.is_empty() {
                return bindings;
            }
            let (classes, ambiguous) = self.class_owners(facts, &file.path, name);
            if !classes.is_empty() && !ambiguous {
                return classes;
            }
            if self.binding_declared(facts, context, name) {
                return OwnerSet::new();
            }
            if let Some(owner) = self.autoload_owner(facts, &file.path, name) {
                return OwnerSet::from([owner]);
            }
            return OwnerSet::new();
        }

        if let Some(parts) = property_chain(expression) {
            let mut owners = if parts.first().is_some_and(|value| value == "self") {
                OwnerSet::from([context.owner_id.clone()])
            } else {
                self.binding_owners(facts, context, &parts[0])
            };
            for property in &parts[1..] {
                let mut next = OwnerSet::new();
                for owner in &owners {
                    next.extend(self.member_owners(
                        facts,
                        owner,
                        property,
                        &mut Default::default(),
                    ));
                    if let Some(nested) = facts
                        .type_by_owner_id
                        .get(owner)
                        .and_then(|values| values.get(property))
                    {
                        next.extend(nested.iter().cloned());
                    }
                }
                owners = next;
                if owners.is_empty() {
                    break;
                }
            }
            return owners;
        }

        OwnerSet::new()
    }

    pub fn infer_type_reference_owners(
        &self,
        facts: &Facts,
        files: &[ParsedFile],
        context: &AnalysisContext,
        tokens: &[Token],
    ) -> OwnerSet {
        let tokens = trim_expression(tokens);
        if let Some(owners) = self.static_load_owners(facts, files, context, tokens) {
            return owners;
        }
        if let Some(name) = simple_identifier(tokens) {
            let file = &files[context.file_index];
            if let Some(owners) = self.preload_alias_owners(facts, &file.path, name) {
                return owners;
            }
            let aliases = self.type_alias_owners(&file.path, name);
            if !aliases.is_empty() {
                return aliases;
            }
            let (owners, ambiguous) = self.class_owners(facts, &file.path, name);
            return if ambiguous { OwnerSet::new() } else { owners };
        }

        let Some(parts) = property_chain(tokens) else {
            return OwnerSet::new();
        };
        if parts.len() < 2 {
            return OwnerSet::new();
        }

        let file = &files[context.file_index];
        let (mut owners, ambiguous) = self.class_owners(facts, &file.path, &parts[0]);
        if ambiguous {
            owners.clear();
        }
        if owners.is_empty() {
            owners = self
                .preload_alias_owners(facts, &file.path, &parts[0])
                .unwrap_or_default();
        }
        if owners.is_empty() {
            owners = self.type_alias_owners(&file.path, &parts[0]);
        }
        for nested in &parts[1..] {
            let mut next = OwnerSet::new();
            for owner in &owners {
                if let Some(values) = facts
                    .type_by_owner_id
                    .get(owner)
                    .and_then(|values| values.get(nested))
                {
                    next.extend(values.iter().cloned());
                }
            }
            owners = next;
            if owners.is_empty() {
                break;
            }
        }
        owners
    }

    fn static_load_owners(
        &self,
        facts: &Facts,
        files: &[ParsedFile],
        context: &AnalysisContext,
        tokens: &[Token],
    ) -> Option<OwnerSet> {
        let tokens = trim_expression(tokens);
        if tokens.len() != 4
            || !matches!(tokens[0].text.as_str(), "preload" | "load")
            || tokens[1].text != "("
            || tokens[2].kind != crate::adapters::gdscript::model::TokenKind::String
            || tokens[3].text != ")"
        {
            return None;
        }
        let path = crate::adapters::gdscript::parser::normalize_import_path(&tokens[2].text)?;
        let path = project_resource_path(&files[context.file_index].project_root, &path);
        Some(
            facts
                .script_owner_by_path
                .get(&path)
                .cloned()
                .map(|owner| OwnerSet::from([owner]))
                .unwrap_or_default(),
        )
    }

    pub fn assignment_member_targets(
        &self,
        facts: &Facts,
        files: &[ParsedFile],
        context: &AnalysisContext,
        tokens: &[Token],
    ) -> Option<(String, OwnerSet)> {
        let tokens = trim_expression(tokens);
        let parts = property_chain(tokens)?;
        if parts.len() < 2 {
            return None;
        }
        let member = parts.last()?.clone();
        let receiver = &tokens[..tokens.len() - 2];
        let owners = if parts.len() == 2 && parts[0] == "self" {
            OwnerSet::from([context.owner_id.clone()])
        } else {
            self.infer_expression_owners(facts, files, context, receiver)
        };
        Some((member, owners))
    }
}

fn terminal_property_access(tokens: &[Token]) -> Option<(&[Token], &str)> {
    let tokens = trim_expression(tokens);
    if tokens.len() < 3
        || tokens[tokens.len() - 2].text != "."
        || tokens.last()?.kind != crate::adapters::gdscript::model::TokenKind::Identifier
    {
        return None;
    }
    let base = trim_expression(&tokens[..tokens.len() - 2]);
    (!base.is_empty()).then_some((base, tokens.last()?.text.as_str()))
}

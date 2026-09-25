mod bindings;
mod callable_maps;
mod callables;
mod calls;
mod expressions;

use std::collections::{BTreeMap, BTreeSet};

use super::facts::Facts;
use super::model::{AnalysisContext, ParsedFile, Token};
use super::syntax::top_level_assignment;

pub type OwnerSet = BTreeSet<String>;
pub type KeyedCallables = BTreeMap<String, OwnerSet>;

#[derive(Debug, Default)]
pub struct SemanticModel {
    members: BTreeMap<String, BTreeMap<String, OwnerSet>>,
    locals: BTreeMap<String, BTreeMap<String, OwnerSet>>,
    returns: BTreeMap<String, OwnerSet>,
    type_aliases: BTreeMap<String, BTreeMap<String, OwnerSet>>,
    member_callables: BTreeMap<String, BTreeMap<String, OwnerSet>>,
    local_callables: BTreeMap<String, BTreeMap<String, OwnerSet>>,
    return_callables: BTreeMap<String, OwnerSet>,
    member_callable_maps: BTreeMap<String, BTreeMap<String, KeyedCallables>>,
    local_callable_maps: BTreeMap<String, BTreeMap<String, KeyedCallables>>,
    return_callable_maps: BTreeMap<String, KeyedCallables>,
}

impl SemanticModel {
    pub fn build(facts: &Facts, files: &[ParsedFile]) -> Self {
        let mut model = Self::default();
        model.seed_declared_types(facts, files);
        for _ in 0..16 {
            let mut changed = false;
            for index in 0..files.len() {
                changed |= model.infer_declarations(facts, files, index);
                changed |= model.infer_statements(facts, files, index);
            }
            for index in 0..files.len() {
                changed |= model.propagate_arguments(facts, files, index);
            }
            if !changed {
                break;
            }
        }
        model
    }

    fn seed_declared_types(&mut self, facts: &Facts, files: &[ParsedFile]) {
        for (file_index, file) in files.iter().enumerate() {
            for declaration in &file.declarations {
                if declaration.kind == "function" {
                    let context = AnalysisContext {
                        file_index,
                        function_id: declaration.node_id.clone(),
                        owner_id: declaration.owner_class_id.clone(),
                    };
                    for name in &declaration.parameter_names {
                        self.add_local(
                            &declaration.node_id,
                            name,
                            self.type_owners(
                                facts,
                                &file.path,
                                declaration
                                    .parameter_types
                                    .get(name)
                                    .map(String::as_str)
                                    .unwrap_or(""),
                            ),
                        );
                        if let Some(default) = declaration.parameter_defaults.get(name) {
                            let owners =
                                self.infer_expression_owners(facts, files, &context, default);
                            let callables =
                                self.infer_expression_callables(facts, files, &context, default);
                            let maps =
                                self.infer_expression_callable_map(facts, files, &context, default);
                            self.add_local(&declaration.node_id, name, owners);
                            self.add_local_callable(&declaration.node_id, name, callables);
                            self.add_local_callable_map(&declaration.node_id, name, maps);
                        }
                    }
                    self.add_return(
                        &declaration.node_id,
                        self.type_owners(facts, &file.path, &declaration.return_type),
                    );
                    continue;
                }

                if !matches!(declaration.kind.as_str(), "variable" | "constant") {
                    continue;
                }
                let owners = self.type_owners(facts, &file.path, &declaration.type_name);
                if !declaration.owner_function.is_empty() {
                    self.add_local(&declaration.owner_function, &declaration.name, owners);
                } else {
                    self.add_member(&declaration.owner_id, &declaration.name, owners);
                }
            }
        }
    }

    fn infer_declarations(
        &mut self,
        facts: &Facts,
        files: &[ParsedFile],
        file_index: usize,
    ) -> bool {
        let file = &files[file_index];
        let mut changed = false;
        for declaration in &file.declarations {
            if declaration.initializer.is_empty()
                || !matches!(declaration.kind.as_str(), "variable" | "constant")
            {
                continue;
            }
            let context = AnalysisContext {
                file_index,
                function_id: declaration.owner_function.clone(),
                owner_id: if declaration.owner_class_id.is_empty() {
                    file.script_owner_id.clone()
                } else {
                    declaration.owner_class_id.clone()
                },
            };
            let owners =
                self.infer_expression_owners(facts, files, &context, &declaration.initializer);
            if declaration.kind == "constant" && declaration.owner_function.is_empty() {
                let aliases = self.infer_type_reference_owners(
                    facts,
                    files,
                    &context,
                    &declaration.initializer,
                );
                changed |= self.add_type_alias(&file.path, &declaration.name, aliases);
            }
            let callables =
                self.infer_expression_callables(facts, files, &context, &declaration.initializer);
            let callable_map = self.infer_expression_callable_map(
                facts,
                files,
                &context,
                &declaration.initializer,
            );
            if !declaration.owner_function.is_empty() {
                changed |= self.add_local(&declaration.owner_function, &declaration.name, owners);
                changed |= self.add_local_callable(
                    &declaration.owner_function,
                    &declaration.name,
                    callables,
                );
                changed |= self.add_local_callable_map(
                    &declaration.owner_function,
                    &declaration.name,
                    callable_map,
                );
            } else {
                changed |= self.add_member(&declaration.owner_id, &declaration.name, owners);
                changed |=
                    self.add_member_callable(&declaration.owner_id, &declaration.name, callables);
                changed |= self.add_member_callable_map(
                    &declaration.owner_id,
                    &declaration.name,
                    callable_map,
                );
            }
        }
        changed
    }

    fn infer_statements(&mut self, facts: &Facts, files: &[ParsedFile], file_index: usize) -> bool {
        let file = &files[file_index];
        let mut changed = false;
        for statement in &file.statements {
            if statement.tokens.is_empty() {
                continue;
            }
            let context = context_for_statement(files, file_index, statement);
            if statement.tokens[0].text == "return" && !context.function_id.is_empty() {
                changed |= self.add_return(
                    &context.function_id,
                    self.infer_expression_owners(facts, files, &context, &statement.tokens[1..]),
                );
                changed |= self.add_return_callable(
                    &context.function_id,
                    self.infer_expression_callables(facts, files, &context, &statement.tokens[1..]),
                );
                changed |= self.add_return_callable_map(
                    &context.function_id,
                    self.infer_expression_callable_map(
                        facts,
                        files,
                        &context,
                        &statement.tokens[1..],
                    ),
                );
            }

            let Some(assignment) = top_level_assignment(&statement.tokens) else {
                continue;
            };
            if super::parser::declaration_for_statement(file, statement).is_some() {
                continue;
            }
            let left = &statement.tokens[..assignment];
            let right = &statement.tokens[assignment + 1..];
            let owners = self.infer_expression_owners(facts, files, &context, right);
            let callables = self.infer_expression_callables(facts, files, &context, right);
            let callable_map = self.infer_expression_callable_map(facts, files, &context, right);

            if let Some((member, target_owners)) =
                self.assignment_member_targets(facts, files, &context, left)
                && !target_owners.is_empty()
            {
                for owner in target_owners {
                    changed |= self.add_member(&owner, &member, owners.clone());
                    changed |= self.add_member_callable(&owner, &member, callables.clone());
                    changed |= self.add_member_callable_map(&owner, &member, callable_map.clone());
                }
                continue;
            }

            let Some((name, force_member)) = assignment_name(left) else {
                continue;
            };
            if context.function_id.is_empty()
                || force_member
                || (self.member_declared(facts, &context.owner_id, &name, &mut BTreeSet::new())
                    && !self.local_declared(facts, &context.function_id, &name))
            {
                changed |= self.add_member(&context.owner_id, &name, owners);
                changed |= self.add_member_callable(&context.owner_id, &name, callables);
                changed |= self.add_member_callable_map(&context.owner_id, &name, callable_map);
            } else {
                changed |= self.add_local(&context.function_id, &name, owners);
                changed |= self.add_local_callable(&context.function_id, &name, callables);
                changed |= self.add_local_callable_map(&context.function_id, &name, callable_map);
            }
        }
        changed
    }

    fn propagate_arguments(
        &mut self,
        facts: &Facts,
        files: &[ParsedFile],
        file_index: usize,
    ) -> bool {
        let file = &files[file_index];
        let mut changed = false;
        for statement in &file.statements {
            let context = context_for_statement(files, file_index, statement);
            for call in super::parser::find_calls(statement, &file.path) {
                let resolution = self.resolve_call(facts, files, &context, &call);
                for target in resolution.function_targets {
                    changed |=
                        self.add_arguments_to_function(facts, files, &context, &target, &call.args);
                }
                for owner in resolution.constructor_owners {
                    for target in self.method_targets(facts, &owner, "_init", false, false) {
                        changed |= self
                            .add_arguments_to_function(facts, files, &context, &target, &call.args);
                    }
                }
            }
        }
        changed
    }

    fn add_arguments_to_function(
        &mut self,
        facts: &Facts,
        files: &[ParsedFile],
        context: &AnalysisContext,
        target: &str,
        args: &[Vec<Token>],
    ) -> bool {
        let Some(declaration) = facts.declaration_by_id.get(target) else {
            return false;
        };
        let mut changed = false;
        for (index, argument) in args.iter().enumerate() {
            let Some(name) = declaration.parameter_names.get(index) else {
                break;
            };
            changed |= self.add_local(
                target,
                name,
                self.infer_expression_owners(facts, files, context, argument),
            );
            changed |= self.add_local_callable(
                target,
                name,
                self.infer_expression_callables(facts, files, context, argument),
            );
            changed |= self.add_local_callable_map(
                target,
                name,
                self.infer_expression_callable_map(facts, files, context, argument),
            );
        }
        changed
    }
}

pub fn context_for_statement(
    files: &[ParsedFile],
    file_index: usize,
    statement: &super::model::Statement,
) -> AnalysisContext {
    context_for_position(
        files,
        file_index,
        statement.start.line,
        statement.indent as u64 + 1,
    )
}

pub fn context_for_position(
    files: &[ParsedFile],
    file_index: usize,
    line: u64,
    column: u64,
) -> AnalysisContext {
    let file = &files[file_index];
    let mut context = AnalysisContext {
        file_index,
        function_id: String::new(),
        owner_id: file.script_owner_id.clone(),
    };
    let mut best = (0_u64, 0_u64, 0_usize);
    let position_indent = column.saturating_sub(1) as usize;

    for declaration in &file.declarations {
        let declaration_position = (
            declaration.span.start_line,
            declaration.span.start_column,
            declaration.indent,
        );
        if declaration.span.start_line > line
            || (declaration.span.start_line == line && declaration.span.start_column >= column)
            || declaration.indent >= position_indent
        {
            continue;
        }
        if declaration.kind == "function" && declaration_position > best {
            context.function_id = declaration.node_id.clone();
            context.owner_id = declaration.owner_class_id.clone();
            best = declaration_position;
        }
    }
    if context.owner_id.is_empty() {
        context.owner_id = file.script_owner_id.clone();
    }
    context
}

fn assignment_name(tokens: &[Token]) -> Option<(String, bool)> {
    let tokens = super::syntax::trim_expression(tokens);
    if tokens.len() == 1 && tokens[0].kind == super::model::TokenKind::Identifier {
        return Some((tokens[0].text.clone(), false));
    }
    if tokens.len() == 3
        && tokens[0].text == "self"
        && tokens[1].text == "."
        && tokens[2].kind == super::model::TokenKind::Identifier
    {
        return Some((tokens[2].text.clone(), true));
    }
    None
}

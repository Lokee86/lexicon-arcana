use std::collections::BTreeSet;

use super::facts::Facts;
use super::model::{AnalysisContext, ParsedFile, Statement, Token, TokenKind};
use super::parser::{declaration_for_statement, is_builtin, is_call_keyword};
use super::relationships::owner_for_statement;
use super::semantic::{SemanticModel, context_for_statement};
use super::syntax::{simple_identifier, trim_expression};

pub fn process(facts: &mut Facts, model: &SemanticModel, files: &[ParsedFile], file_index: usize) {
    let file = &files[file_index];

    for declaration in &file.declarations {
        if declaration.kind != "function" || declaration.node_id.is_empty() {
            continue;
        }
        for name in &declaration.parameter_names {
            ensure_parameter(facts, file, declaration, name);
        }
    }

    for statement in &file.statements {
        let declaration = declaration_for_statement(file, statement).cloned();
        let context = context_for_statement(files, file_index, statement);
        let mut source = owner_for_statement(files, file_index, statement);
        if let Some(declaration) = &declaration
            && !declaration.owner_function.is_empty()
        {
            source = declaration.owner_function.clone();
        }
        if source.is_empty() {
            continue;
        }

        if let Some(declaration) = declaration
            && matches!(declaration.kind.as_str(), "variable" | "constant")
        {
            if !declaration.node_id.is_empty() && !declaration.initializer.is_empty() {
                facts.add_dataflow_edge(
                    &source,
                    &declaration.node_id,
                    "writes",
                    Some(declaration.span.clone()),
                );
                emit_reads(
                    facts,
                    model,
                    files,
                    &context,
                    &source,
                    file,
                    &declaration.initializer,
                    &[],
                );
            }
            continue;
        }

        emit_statement(facts, model, files, &context, &source, file, statement);
    }
}

fn ensure_parameter(
    facts: &mut Facts,
    file: &ParsedFile,
    function: &super::model::Declaration,
    name: &str,
) -> String {
    let identity = format!("{}::parameter::{name}", function.key);
    let id = facts.add_node(
        "parameter",
        name,
        &file.path,
        &format!("{}::{name}", function.key),
        &identity,
        Some(function.span.clone()),
        None,
        None,
    );
    facts.add_edge(
        &function.node_id,
        &id,
        "defines",
        Some(function.span.clone()),
        None,
    );
    id
}

fn emit_statement(
    facts: &mut Facts,
    model: &SemanticModel,
    files: &[ParsedFile],
    context: &AnalysisContext,
    source: &str,
    file: &ParsedFile,
    statement: &Statement,
) {
    if let Some(assignment) = assignment_index(&statement.tokens) {
        let left = trim_expression(&statement.tokens[..assignment]);
        let compound = !matches!(statement.tokens[assignment].text.as_str(), "=" | ":=");
        if let Some((receiver, name)) = target(left) {
            if let Some(receiver) = receiver {
                emit_member_target(
                    facts, model, files, context, source, file, left, receiver, name, compound,
                );
            } else if let Some(first) = left.first()
                && let Some(id) = local_target(facts, &context.function_id, name, first)
            {
                if compound {
                    facts.add_dataflow_edge(
                        source,
                        &id,
                        "reads",
                        Some(super::lexer::span(&file.path, first, left.last().unwrap())),
                    );
                }
                facts.add_dataflow_edge(
                    source,
                    &id,
                    "writes",
                    Some(super::lexer::span(&file.path, first, left.last().unwrap())),
                );
            }
        }
        emit_reads(
            facts,
            model,
            files,
            context,
            source,
            file,
            &statement.tokens[assignment + 1..],
            left,
        );
        return;
    }

    if statement.tokens.len() > 1
        && (matches!(statement.tokens[0].text.as_str(), "++" | "--")
            || matches!(statement.tokens.last().unwrap().text.as_str(), "++" | "--"))
    {
        let target_tokens = if matches!(statement.tokens.last().unwrap().text.as_str(), "++" | "--")
        {
            &statement.tokens[..statement.tokens.len() - 1]
        } else {
            &statement.tokens[1..]
        };
        if let Some((receiver, name)) = target(target_tokens) {
            if let Some(receiver) = receiver {
                emit_member_target(
                    facts,
                    model,
                    files,
                    context,
                    source,
                    file,
                    target_tokens,
                    receiver,
                    name,
                    true,
                );
            } else if let Some(id) =
                local_target(facts, &context.function_id, name, &target_tokens[0])
            {
                let span = super::lexer::span(
                    &file.path,
                    &target_tokens[0],
                    target_tokens.last().unwrap(),
                );
                facts.add_dataflow_edge(source, &id, "reads", Some(span.clone()));
                facts.add_dataflow_edge(source, &id, "writes", Some(span));
            }
        }
        return;
    }

    emit_reads(
        facts,
        model,
        files,
        context,
        source,
        file,
        &statement.tokens,
        &[],
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_reads(
    facts: &mut Facts,
    model: &SemanticModel,
    files: &[ParsedFile],
    context: &AnalysisContext,
    source: &str,
    file: &ParsedFile,
    tokens: &[Token],
    excluded: &[Token],
) {
    for (index, current) in tokens.iter().enumerate() {
        if current.kind != TokenKind::Identifier
            || is_call_keyword(&current.text)
            || is_builtin(&current.text)
            || current.text == "self"
            || excluded.iter().any(|candidate| {
                candidate.line == current.line && candidate.column == current.column
            })
            || (index > 0 && tokens[index - 1].text == "func")
        {
            continue;
        }

        if index > 0 && tokens[index - 1].text == "." {
            let base = &tokens[..index - 1];
            if let Some(id) = member_target_id(model, facts, files, context, base, &current.text) {
                facts.add_dataflow_edge(
                    source,
                    &id,
                    "reads",
                    Some(super::lexer::span(&file.path, current, current)),
                );
            }
            continue;
        }

        if let Some(id) = local_target(facts, &context.function_id, &current.text, current) {
            facts.add_dataflow_edge(
                source,
                &id,
                "reads",
                Some(super::lexer::span(&file.path, current, current)),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_member_target(
    facts: &mut Facts,
    model: &SemanticModel,
    files: &[ParsedFile],
    context: &AnalysisContext,
    source: &str,
    file: &ParsedFile,
    target: &[Token],
    receiver: &[Token],
    name: &str,
    compound: bool,
) {
    let Some(id) = member_target_id(model, facts, files, context, receiver, name) else {
        return;
    };
    let span = super::lexer::span(&file.path, &target[0], target.last().unwrap());
    if compound {
        facts.add_dataflow_edge(source, &id, "reads", Some(span.clone()));
    }
    facts.add_dataflow_edge(source, &id, "writes", Some(span));
}

fn local_target(facts: &Facts, function_id: &str, name: &str, reference: &Token) -> Option<String> {
    if function_id.is_empty() {
        return None;
    }
    let mut best: Option<(u64, u64, String)> = None;
    let mut ambiguous = false;
    for id in facts.local_value_ids(function_id, name) {
        let Some(declaration) = facts.declaration_by_id.get(id) else {
            continue;
        };
        let position = (declaration.span.start_line, declaration.span.start_column);
        if contains_token(declaration, reference)
            || position.0 > reference.line
            || (position.0 == reference.line && position.1 >= reference.column)
        {
            continue;
        }
        match &best {
            None => {
                best = Some((position.0, position.1, id.clone()));
                ambiguous = false;
            }
            Some((line, column, _)) if position > (*line, *column) => {
                best = Some((position.0, position.1, id.clone()));
                ambiguous = false;
            }
            Some((line, column, best_id)) if position == (*line, *column) && best_id != id => {
                ambiguous = true;
            }
            _ => {}
        }
    }
    if let Some((_, _, id)) = best
        && !ambiguous
    {
        return Some(id);
    }

    let function = facts.declaration_by_id.get(function_id)?;
    function
        .parameter_names
        .iter()
        .any(|parameter| parameter == name)
        .then(|| {
            crate::node_id(
                "gdscript",
                "parameter",
                &format!("{}::parameter::{name}", function.key),
            )
        })
}

fn contains_token(declaration: &super::model::Declaration, reference: &Token) -> bool {
    if reference.line < declaration.span.start_line || reference.line > declaration.span.end_line {
        return false;
    }
    if reference.line == declaration.span.start_line
        && reference.column < declaration.span.start_column
    {
        return false;
    }
    reference.line != declaration.span.end_line || reference.column < declaration.span.end_column
}

fn member_target_id(
    model: &SemanticModel,
    facts: &Facts,
    files: &[ParsedFile],
    context: &AnalysisContext,
    receiver: &[Token],
    name: &str,
) -> Option<String> {
    let owners = if simple_identifier(receiver) == Some("self") {
        if !context.owner_id.is_empty() {
            BTreeSet::from([context.owner_id.clone()])
        } else {
            BTreeSet::new()
        }
    } else {
        model.infer_expression_owners(facts, files, context, receiver)
    };

    let candidates = owners
        .into_iter()
        .flat_map(|owner| facts.member_value_ids(&owner, name).iter().cloned())
        .collect::<BTreeSet<_>>();
    (candidates.len() == 1).then(|| candidates.into_iter().next().unwrap())
}

fn target(tokens: &[Token]) -> Option<(Option<&[Token]>, &str)> {
    if tokens.len() == 1 && tokens[0].kind == TokenKind::Identifier {
        return Some((None, tokens[0].text.as_str()));
    }
    if tokens.len() >= 3
        && tokens[tokens.len() - 2].text == "."
        && tokens.last()?.kind == TokenKind::Identifier
    {
        return Some((
            Some(&tokens[..tokens.len() - 2]),
            tokens.last()?.text.as_str(),
        ));
    }
    None
}

fn assignment_index(tokens: &[Token]) -> Option<usize> {
    let mut depth = 0_i32;
    for (index, token) in tokens.iter().enumerate() {
        match token.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" if depth > 0 => depth -= 1,
            "=" | ":=" | "+=" | "-=" | "*=" | "/=" if depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

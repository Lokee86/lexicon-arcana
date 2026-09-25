use rustpython_parser::ast;

use super::super::facts::Facts;
use super::super::model::{Repository, SourceFile};
use super::super::source::{byte_location, generated, span};

pub fn emit(repository: &Repository, facts: &mut Facts) {
    for file in &repository.files {
        let Some(suite) = &file.suite else { continue };
        if generated(&file.source) {
            continue;
        }
        walk_suite(file, suite, None, facts);
    }
}

fn walk_suite(
    file: &SourceFile,
    suite: &ast::Suite,
    inherited_if_suffix: Option<&[ast::Stmt]>,
    facts: &mut Facts,
) {
    for (index, statement) in suite.iter().enumerate() {
        let local_suffix = &suite[index + 1..];
        let downstream = if local_suffix.is_empty() {
            inherited_if_suffix.unwrap_or(&[])
        } else {
            local_suffix
        };

        match statement {
            ast::Stmt::FunctionDef(value) => walk_suite(file, &value.body, None, facts),
            ast::Stmt::AsyncFunctionDef(value) => walk_suite(file, &value.body, None, facts),
            ast::Stmt::ClassDef(value) => walk_suite(file, &value.body, None, facts),
            ast::Stmt::If(value) => {
                walk_suite(file, &value.body, Some(downstream), facts);
                walk_suite(file, &value.orelse, Some(downstream), facts);
            }
            ast::Stmt::For(value) => {
                walk_suite(file, &value.body, None, facts);
                walk_suite(file, &value.orelse, None, facts);
            }
            ast::Stmt::AsyncFor(value) => {
                walk_suite(file, &value.body, None, facts);
                walk_suite(file, &value.orelse, None, facts);
            }
            ast::Stmt::While(value) => {
                walk_suite(file, &value.body, None, facts);
                walk_suite(file, &value.orelse, None, facts);
            }
            ast::Stmt::With(value) => walk_suite(file, &value.body, None, facts),
            ast::Stmt::AsyncWith(value) => walk_suite(file, &value.body, None, facts),
            ast::Stmt::Try(value) => {
                emit_empty_handler_flows(file, &value.handlers, downstream, facts);
                walk_suite(file, &value.body, None, facts);
                for handler in &value.handlers {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    walk_suite(file, &handler.body, None, facts);
                }
                walk_suite(file, &value.orelse, None, facts);
                walk_suite(file, &value.finalbody, None, facts);
            }
            ast::Stmt::TryStar(value) => {
                emit_empty_handler_flows(file, &value.handlers, downstream, facts);
                walk_suite(file, &value.body, None, facts);
                for handler in &value.handlers {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    walk_suite(file, &handler.body, None, facts);
                }
                walk_suite(file, &value.orelse, None, facts);
                walk_suite(file, &value.finalbody, None, facts);
            }
            ast::Stmt::Match(value) => {
                for case in &value.cases {
                    walk_suite(file, &case.body, None, facts);
                }
            }
            _ => {}
        }
    }
}

fn emit_empty_handler_flows(
    file: &SourceFile,
    handlers: &[ast::ExceptHandler],
    downstream: &[ast::Stmt],
    facts: &mut Facts,
) {
    for handler in handlers {
        let ast::ExceptHandler::ExceptHandler(handler) = handler;
        if handler.body.is_empty()
            || !handler
                .body
                .iter()
                .all(|statement| matches!(statement, ast::Stmt::Pass(_)))
        {
            continue;
        }
        let Some((flow, evidence)) = classify(downstream) else {
            continue;
        };

        let (line, column) = byte_location(handler, &file.source);
        let handler_identity = format!(
            "@semantic/error-handler/python/{}:{line}:{column}",
            file.relative
        );
        let (evidence_line, evidence_column) = byte_location(evidence, &file.source);
        let identity = format!("{handler_identity}/flow-{flow}:{evidence_line}:{evidence_column}");
        let record_span = span(evidence, &file.relative, &file.source);
        let id = facts.add_node(
            "protocol",
            &format!("error-flow:{flow}"),
            &file.relative,
            &identity,
            Some(&identity),
            record_span.clone(),
            None,
            None,
        );
        let handler_id = crate::node_id("python", "protocol", &handler_identity);
        facts.add_edge(&handler_id, &id, "contains", record_span, None);
    }
}

fn classify(statements: &[ast::Stmt]) -> Option<(&'static str, &ast::Stmt)> {
    let first = statements.first()?;
    match first {
        ast::Stmt::Raise(_) => Some(("enclosing-propagation", first)),
        ast::Stmt::Return(_) | ast::Stmt::Try(_) | ast::Stmt::TryStar(_) => {
            Some(("fallback", first))
        }
        ast::Stmt::Assign(_) | ast::Stmt::AnnAssign(_) | ast::Stmt::AugAssign(_)
            if statements
                .get(1)
                .is_some_and(|next| matches!(next, ast::Stmt::Return(_))) =>
        {
            Some(("fallback", first))
        }
        _ => Some(("continuation", first)),
    }
}

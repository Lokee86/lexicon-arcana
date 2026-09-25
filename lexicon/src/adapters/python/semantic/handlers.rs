use std::collections::BTreeMap;

use rustpython_parser::ast;

use super::super::facts::Facts;
use super::super::model::SourceFile;
use super::super::source::{byte_location, span};
use super::{handler_identity, handler_node};

const RECORDING_TARGETS: &[&str] = &[
    "capture_error",
    "capture_exception",
    "critical",
    "debug",
    "error",
    "exception",
    "info",
    "log",
    "report_error",
    "warn",
    "warning",
];

#[derive(Clone)]
struct Evidence {
    line: u64,
    column: u64,
    span: Option<crate::SourceSpan>,
}

pub fn emit_handlers(file: &SourceFile, facts: &mut Facts) {
    let Some(suite) = &file.suite else {
        return;
    };
    walk_suite(file, suite, facts);
}

fn walk_suite(file: &SourceFile, suite: &ast::Suite, facts: &mut Facts) {
    for statement in suite {
        match statement {
            ast::Stmt::Try(value) => {
                emit_try_handlers(file, &value.handlers, facts);
                walk_suite(file, &value.body, facts);
                walk_handlers(file, &value.handlers, facts);
                walk_suite(file, &value.orelse, facts);
                walk_suite(file, &value.finalbody, facts);
            }
            ast::Stmt::TryStar(value) => {
                emit_try_handlers(file, &value.handlers, facts);
                walk_suite(file, &value.body, facts);
                walk_handlers(file, &value.handlers, facts);
                walk_suite(file, &value.orelse, facts);
                walk_suite(file, &value.finalbody, facts);
            }
            ast::Stmt::FunctionDef(value) => walk_suite(file, &value.body, facts),
            ast::Stmt::AsyncFunctionDef(value) => walk_suite(file, &value.body, facts),
            ast::Stmt::ClassDef(value) => walk_suite(file, &value.body, facts),
            ast::Stmt::If(value) => {
                walk_suite(file, &value.body, facts);
                walk_suite(file, &value.orelse, facts);
            }
            ast::Stmt::For(value) => {
                walk_suite(file, &value.body, facts);
                walk_suite(file, &value.orelse, facts);
            }
            ast::Stmt::AsyncFor(value) => {
                walk_suite(file, &value.body, facts);
                walk_suite(file, &value.orelse, facts);
            }
            ast::Stmt::While(value) => {
                walk_suite(file, &value.body, facts);
                walk_suite(file, &value.orelse, facts);
            }
            ast::Stmt::With(value) => walk_suite(file, &value.body, facts),
            ast::Stmt::AsyncWith(value) => walk_suite(file, &value.body, facts),
            ast::Stmt::Match(value) => {
                for case in &value.cases {
                    walk_suite(file, &case.body, facts);
                }
            }
            _ => {}
        }
    }
}

fn walk_handlers(file: &SourceFile, handlers: &[ast::ExceptHandler], facts: &mut Facts) {
    for handler in handlers {
        let ast::ExceptHandler::ExceptHandler(handler) = handler;
        walk_suite(file, &handler.body, facts);
    }
}

fn emit_try_handlers(file: &SourceFile, handlers: &[ast::ExceptHandler], facts: &mut Facts) {
    for handler in handlers {
        let ast::ExceptHandler::ExceptHandler(handler) = handler;
        let handler_id = handler_node(file, handler, facts);
        let mut actions = BTreeMap::new();
        collect_suite(file, &handler.body, &mut actions);
        let (handler_identity, _, _) = handler_identity(file, handler);

        for action in ["propagate", "record", "recover"] {
            let Some(evidence) = actions.get(action) else {
                continue;
            };
            let identity = format!(
                "{handler_identity}/{action}:{}:{}",
                evidence.line, evidence.column
            );
            let action_id = facts.add_node(
                "protocol",
                &format!("error-action:{action}"),
                &file.relative,
                &identity,
                Some(&identity),
                evidence.span.clone(),
                None,
                None,
            );
            facts.add_edge(
                &handler_id,
                &action_id,
                "contains",
                evidence.span.clone(),
                None,
            );
        }
    }
}

fn collect_suite(
    file: &SourceFile,
    suite: &ast::Suite,
    actions: &mut BTreeMap<&'static str, Evidence>,
) {
    for statement in suite {
        collect_statement(file, statement, actions);
    }
}

fn collect_statement(
    file: &SourceFile,
    statement: &ast::Stmt,
    actions: &mut BTreeMap<&'static str, Evidence>,
) {
    match statement {
        ast::Stmt::FunctionDef(_) | ast::Stmt::AsyncFunctionDef(_) | ast::Stmt::ClassDef(_) => {}
        ast::Stmt::Raise(value) => {
            record(file, value, "propagate", actions);
        }
        ast::Stmt::Return(value) => {
            record(file, value, "recover", actions);
            if let Some(expression) = value.value.as_deref() {
                collect_expression(file, expression, actions);
            }
        }
        ast::Stmt::Assign(value) => {
            record(file, value, "recover", actions);
            collect_expression(file, &value.value, actions);
        }
        ast::Stmt::AnnAssign(value) => {
            record(file, value, "recover", actions);
            if let Some(expression) = value.value.as_deref() {
                collect_expression(file, expression, actions);
            }
        }
        ast::Stmt::AugAssign(value) => {
            record(file, value, "recover", actions);
            collect_expression(file, &value.value, actions);
        }
        ast::Stmt::Import(value) => record(file, value, "recover", actions),
        ast::Stmt::ImportFrom(value) => record(file, value, "recover", actions),
        ast::Stmt::Break(value) => record(file, value, "recover", actions),
        ast::Stmt::Continue(value) => record(file, value, "recover", actions),
        ast::Stmt::Expr(value) => collect_expression(file, &value.value, actions),
        ast::Stmt::If(value) => {
            collect_expression(file, &value.test, actions);
            collect_suite(file, &value.body, actions);
            collect_suite(file, &value.orelse, actions);
        }
        ast::Stmt::For(value) => {
            collect_expression(file, &value.iter, actions);
            collect_suite(file, &value.body, actions);
            collect_suite(file, &value.orelse, actions);
        }
        ast::Stmt::AsyncFor(value) => {
            collect_expression(file, &value.iter, actions);
            collect_suite(file, &value.body, actions);
            collect_suite(file, &value.orelse, actions);
        }
        ast::Stmt::While(value) => {
            collect_expression(file, &value.test, actions);
            collect_suite(file, &value.body, actions);
            collect_suite(file, &value.orelse, actions);
        }
        ast::Stmt::Try(value) => {
            collect_suite(file, &value.body, actions);
            collect_suite(file, &value.orelse, actions);
            collect_suite(file, &value.finalbody, actions);
        }
        ast::Stmt::TryStar(value) => {
            collect_suite(file, &value.body, actions);
            collect_suite(file, &value.orelse, actions);
            collect_suite(file, &value.finalbody, actions);
        }
        ast::Stmt::With(value) => collect_suite(file, &value.body, actions),
        ast::Stmt::AsyncWith(value) => collect_suite(file, &value.body, actions),
        _ => {}
    }
}

fn collect_expression(
    file: &SourceFile,
    expression: &ast::Expr,
    actions: &mut BTreeMap<&'static str, Evidence>,
) {
    match expression {
        ast::Expr::NamedExpr(value) => {
            record(file, value, "recover", actions);
            collect_expression(file, &value.value, actions);
        }
        ast::Expr::Call(value) => {
            let action = if call_leaf(&value.func)
                .is_some_and(|leaf| RECORDING_TARGETS.contains(&leaf.as_str()))
            {
                "record"
            } else {
                "recover"
            };
            record(file, value, action, actions);
            collect_expression(file, &value.func, actions);
            for argument in &value.args {
                collect_expression(file, argument, actions);
            }
            for keyword in &value.keywords {
                collect_expression(file, &keyword.value, actions);
            }
        }
        ast::Expr::Attribute(value) => collect_expression(file, &value.value, actions),
        ast::Expr::BoolOp(value) => {
            for item in &value.values {
                collect_expression(file, item, actions);
            }
        }
        ast::Expr::BinOp(value) => {
            collect_expression(file, &value.left, actions);
            collect_expression(file, &value.right, actions);
        }
        ast::Expr::UnaryOp(value) => collect_expression(file, &value.operand, actions),
        ast::Expr::IfExp(value) => {
            collect_expression(file, &value.test, actions);
            collect_expression(file, &value.body, actions);
            collect_expression(file, &value.orelse, actions);
        }
        ast::Expr::Await(value) => collect_expression(file, &value.value, actions),
        _ => {}
    }
}

fn record<T: ast::Ranged>(
    file: &SourceFile,
    node: &T,
    action: &'static str,
    actions: &mut BTreeMap<&'static str, Evidence>,
) {
    actions.entry(action).or_insert_with(|| {
        let (line, column) = byte_location(node, &file.source);
        Evidence {
            line,
            column,
            span: span(node, &file.relative, &file.source),
        }
    });
}

fn call_leaf(expression: &ast::Expr) -> Option<String> {
    match expression {
        ast::Expr::Name(value) => Some(value.id.to_lowercase()),
        ast::Expr::Attribute(value) => Some(value.attr.to_lowercase()),
        _ => None,
    }
}

mod error_flow;
mod handlers;
mod outcomes;

use rustpython_parser::ast;

use super::facts::Facts;
use super::model::{CallInfo, SourceFile};
use super::source::{byte_location, generated, span};

const CAPABILITIES: &str = "control-flow,error-handling,calls,source-spans,outcome-obligations";

pub fn emit_file_facts(file: &SourceFile, facts: &mut Facts) {
    if file.suite.is_none() || generated(&file.source) {
        return;
    }
    let identity = format!("@semantic/capabilities/python/{}", file.relative);
    facts.add_node(
        "protocol",
        &format!("semantic-capabilities:python:{CAPABILITIES}"),
        &file.relative,
        &identity,
        Some(&identity),
        None,
        None,
        None,
    );
    handlers::emit_handlers(file, facts);
    error_flow::emit_file(file, facts);
}

pub fn emit_outcome_facts(calls: &[CallInfo], facts: &mut Facts) {
    outcomes::emit(calls, facts);
}

pub(super) fn handler_identity(
    file: &SourceFile,
    handler: &ast::ExceptHandlerExceptHandler,
) -> (String, u64, u64) {
    let (line, column) = byte_location(handler, file);
    (
        format!(
            "@semantic/error-handler/python/{}:{line}:{column}",
            file.relative
        ),
        line,
        column,
    )
}

pub(super) fn handler_node(
    file: &SourceFile,
    handler: &ast::ExceptHandlerExceptHandler,
    facts: &mut Facts,
) -> String {
    let (identity, _, _) = handler_identity(file, handler);
    facts.add_node(
        "protocol",
        "error-handler:python",
        &file.relative,
        &identity,
        Some(&identity),
        span(handler, file),
        None,
        None,
    )
}

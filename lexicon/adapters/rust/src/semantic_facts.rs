use crate::model::{Context, SourceFile};
use crate::paths::{span_start, span_value};
use proc_macro2::Span;
use std::collections::BTreeMap;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

const CAPABILITIES: &str = "control-flow,error-handling,calls,source-spans,outcome-obligations";

use crate::semantic_actions::{actions_for_block, actions_for_expr, ErrorAction};

struct Handler {
    span: Span,
    actions: Vec<(ErrorAction, Span)>,
}

pub(crate) fn emit(context: &mut Context) {
    let sources: Vec<SourceFile> = context.sources.values().cloned().collect();
    for source in sources {
        emit_capabilities(context, &source);
        let mut collector = HandlerCollector::default();
        collector.visit_file(&source.syntax);
        for handler in collector.handlers {
            emit_handler(context, &source, handler);
        }
    }
}

fn emit_capabilities(context: &mut Context, source: &SourceFile) {
    let identity = format!("@semantic/capabilities/rust/{}", source.relative);
    context.facts.add_node(
        "rust",
        "protocol",
        &identity,
        &format!("semantic-capabilities:rust:{CAPABILITIES}"),
        &source.relative,
        &identity,
        None,
        None,
        BTreeMap::new(),
    );
}

fn emit_handler(context: &mut Context, source: &SourceFile, handler: Handler) {
    let (line, column) = span_start(handler.span);
    let identity = format!(
        "@semantic/error-handler/rust/{}:{line}:{column}",
        source.relative
    );
    let handler_id = context.facts.add_node(
        "rust",
        "protocol",
        &identity,
        "error-handler:rust",
        &source.relative,
        &identity,
        None,
        span_value(handler.span, &source.relative),
        BTreeMap::new(),
    );
    for (action, span) in handler.actions {
        let (action_line, action_column) = span_start(span);
        let action_identity = format!("{identity}/{}:{action_line}:{action_column}", action.name());
        let action_id = context.facts.add_node(
            "rust",
            "protocol",
            &action_identity,
            &format!("error-action:{}", action.name()),
            &source.relative,
            &action_identity,
            None,
            span_value(span, &source.relative),
            BTreeMap::new(),
        );
        context.facts.add_edge(
            &handler_id,
            &action_id,
            "contains",
            span_value(span, &source.relative),
        );
    }
}

#[derive(Default)]
struct HandlerCollector {
    handlers: Vec<Handler>,
}

impl<'ast> Visit<'ast> for HandlerCollector {
    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        for arm in &node.arms {
            if is_err_pattern(&arm.pat) {
                self.handlers.push(Handler {
                    span: arm.span(),
                    actions: actions_for_expr(&arm.body),
                });
            }
        }
        visit::visit_expr_match(self, node);
    }

    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        if let syn::Expr::Let(condition) = node.cond.as_ref() {
            if is_err_pattern(&condition.pat) {
                self.handlers.push(Handler {
                    span: node.then_branch.span(),
                    actions: actions_for_block(&node.then_branch),
                });
            }
        }
        visit::visit_expr_if(self, node);
    }
}

fn is_err_pattern(pattern: &syn::Pat) -> bool {
    match pattern {
        syn::Pat::TupleStruct(value) => value
            .path
            .segments
            .last()
            .is_some_and(|part| part.ident == "Err"),
        syn::Pat::Path(value) => value
            .path
            .segments
            .last()
            .is_some_and(|part| part.ident == "Err"),
        syn::Pat::Or(value) => value.cases.iter().any(is_err_pattern),
        syn::Pat::Paren(value) => is_err_pattern(&value.pat),
        syn::Pat::Reference(value) => is_err_pattern(&value.pat),
        syn::Pat::Type(value) => is_err_pattern(&value.pat),
        _ => false,
    }
}

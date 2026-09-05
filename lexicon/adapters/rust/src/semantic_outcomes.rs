use crate::model::{Context, SourceFile};
use crate::paths::{span_start, span_value};
use proc_macro2::Span;
use std::collections::{BTreeMap, BTreeSet};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

struct Operation {
    span: Span,
    consumed: bool,
}

pub(crate) fn emit(context: &mut Context) {
    let proven_spans = proven_fallible_call_spans(context);
    if proven_spans.is_empty() {
        return;
    }
    let sources: Vec<SourceFile> = context.sources.values().cloned().collect();
    for source in sources {
        if crate::semantic_facts::is_generated_semantic_source(&source) {
            continue;
        }
        let mut collector = OutcomeCollector {
            path: &source.relative,
            proven_spans: &proven_spans,
            operations: Vec::new(),
        };
        collector.visit_file(&source.syntax);
        for operation in collector.operations {
            emit_operation(context, &source, operation);
        }
    }
}

fn proven_fallible_call_spans(context: &Context) -> BTreeSet<String> {
    let fallible_ids: BTreeSet<_> = context
        .functions
        .values()
        .filter(|function| function.return_type.as_deref().is_some_and(is_result_type))
        .map(|function| function.id.as_str())
        .collect();
    context
        .facts
        .edges
        .values()
        .filter_map(|edge| {
            let target = edge.get("target")?.as_str()?;
            if edge.get("relation")?.as_str()? != "calls" || !fallible_ids.contains(target) {
                return None;
            }
            edge.get("span").map(ToString::to_string)
        })
        .collect()
}

fn is_result_type(value: &str) -> bool {
    let compact = value.replace(' ', "");
    compact.starts_with("Result<") || compact.contains("::Result<")
}

fn emit_operation(context: &mut Context, source: &SourceFile, operation: Operation) {
    let (line, column) = span_start(operation.span);
    let identity = format!(
        "@semantic/outcome-operation/rust/{}:{line}:{column}",
        source.relative
    );
    let operation_span = span_value(operation.span, &source.relative);
    let operation_id = context.facts.add_node(
        "rust",
        "protocol",
        &identity,
        "outcome-operation:rust:fallible",
        &source.relative,
        &identity,
        None,
        operation_span.clone(),
        BTreeMap::new(),
    );
    if !operation.consumed {
        return;
    }
    let action_identity = format!("{identity}/consume:{line}:{column}");
    let action_id = context.facts.add_node(
        "rust",
        "protocol",
        &action_identity,
        "outcome-action:consume",
        &source.relative,
        &action_identity,
        None,
        operation_span.clone(),
        BTreeMap::new(),
    );
    context
        .facts
        .add_edge(&operation_id, &action_id, "contains", operation_span);
}

struct OutcomeCollector<'a> {
    path: &'a str,
    proven_spans: &'a BTreeSet<String>,
    operations: Vec<Operation>,
}

impl OutcomeCollector<'_> {
    fn is_fallible_span(&self, span: Span) -> bool {
        span_value(span, self.path)
            .map(|value| self.proven_spans.contains(&value.to_string()))
            .unwrap_or(false)
    }
}

impl<'ast> Visit<'ast> for OutcomeCollector<'_> {
    fn visit_stmt(&mut self, node: &'ast syn::Stmt) {
        if let syn::Stmt::Expr(expression, Some(_)) = node {
            match expression {
                syn::Expr::Call(call) if self.is_fallible_span(call.span()) => {
                    self.operations.push(Operation {
                        span: call.span(),
                        consumed: false,
                    });
                    self.visit_expr(&call.func);
                    for argument in &call.args {
                        self.visit_expr(argument);
                    }
                    return;
                }
                syn::Expr::MethodCall(call) if self.is_fallible_span(call.span()) => {
                    self.operations.push(Operation {
                        span: call.span(),
                        consumed: false,
                    });
                    self.visit_expr(&call.receiver);
                    for argument in &call.args {
                        self.visit_expr(argument);
                    }
                    return;
                }
                _ => {}
            }
        }
        visit::visit_stmt(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if self.is_fallible_span(node.span()) {
            self.operations.push(Operation {
                span: node.span(),
                consumed: true,
            });
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if self.is_fallible_span(node.span()) {
            self.operations.push(Operation {
                span: node.span(),
                consumed: true,
            });
        }
        visit::visit_expr_method_call(self, node);
    }
}

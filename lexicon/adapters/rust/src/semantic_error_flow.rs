use crate::semantic_actions::{actions_for_block, actions_for_expr};
use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

pub(crate) struct FlowEvidence {
    pub(crate) handler_span: Span,
    pub(crate) flow: &'static str,
    pub(crate) evidence_span: Span,
}

pub(crate) fn collect(file: &syn::File) -> Vec<FlowEvidence> {
    let mut collector = FlowCollector::default();
    collector.visit_file(file);
    collector.flows
}

#[derive(Default)]
struct FlowCollector {
    flows: Vec<FlowEvidence>,
}

impl<'ast> Visit<'ast> for FlowCollector {
    fn visit_block(&mut self, node: &'ast syn::Block) {
        visit::visit_block(self, node);
        for pair in node.stmts.windows(2) {
            let Some(current) = stmt_expr(&pair[0]) else {
                continue;
            };
            let Some((flow, evidence_span)) = classify_following_flow(&pair[1]) else {
                continue;
            };
            match current {
                syn::Expr::Match(value) => {
                    for arm in &value.arms {
                        if is_err_pattern(&arm.pat) && actions_for_expr(&arm.body).is_empty() {
                            self.flows.push(FlowEvidence {
                                handler_span: arm.span(),
                                flow,
                                evidence_span,
                            });
                        }
                    }
                }
                syn::Expr::If(value) => {
                    if let syn::Expr::Let(condition) = value.cond.as_ref() {
                        if is_err_pattern(&condition.pat)
                            && actions_for_block(&value.then_branch).is_empty()
                        {
                            self.flows.push(FlowEvidence {
                                handler_span: value.then_branch.span(),
                                flow,
                                evidence_span,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn stmt_expr(statement: &syn::Stmt) -> Option<&syn::Expr> {
    match statement {
        syn::Stmt::Expr(expression, _) => Some(expression),
        _ => None,
    }
}

fn classify_following_flow(statement: &syn::Stmt) -> Option<(&'static str, Span)> {
    match statement {
        syn::Stmt::Expr(syn::Expr::Return(value), _) => {
            let flow = if value.expr.as_deref().is_some_and(is_err_expression) {
                "enclosing-propagation"
            } else {
                "fallback"
            };
            Some((flow, value.span()))
        }
        syn::Stmt::Expr(syn::Expr::Try(value), _) => Some(("enclosing-propagation", value.span())),
        syn::Stmt::Expr(value, _) => Some(("continuation", value.span())),
        syn::Stmt::Local(value) => Some(("continuation", value.span())),
        _ => None,
    }
}

fn is_err_expression(expression: &syn::Expr) -> bool {
    matches!(
        expression,
        syn::Expr::Call(call)
            if matches!(call.func.as_ref(), syn::Expr::Path(path) if path.path.segments.last().is_some_and(|part| part.ident == "Err"))
    )
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

use proc_macro2::Span;
use std::collections::BTreeMap;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ErrorAction {
    Propagate,
    Record,
    Recover,
}

impl ErrorAction {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Propagate => "propagate",
            Self::Record => "record",
            Self::Recover => "recover",
        }
    }
}

#[derive(Default)]
struct ActionCollector {
    actions: BTreeMap<ErrorAction, Span>,
}

impl ActionCollector {
    fn record(&mut self, action: ErrorAction, span: Span) {
        self.actions.entry(action).or_insert(span);
    }

    fn record_macro(&mut self, node: &syn::Macro, span: Span) {
        let name = node.path.segments.last().map(|part| part.ident.to_string());
        if name.as_deref().is_some_and(is_recording_target) {
            self.record(ErrorAction::Record, span);
        }
    }
}

impl<'ast> Visit<'ast> for ActionCollector {
    fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}

    fn visit_item(&mut self, _: &'ast syn::Item) {}

    fn visit_expr_match(&mut self, _: &'ast syn::ExprMatch) {}

    fn visit_expr_if(&mut self, _: &'ast syn::ExprIf) {}

    fn visit_expr_try(&mut self, node: &'ast syn::ExprTry) {
        self.record(ErrorAction::Propagate, node.span());
    }

    fn visit_expr_return(&mut self, node: &'ast syn::ExprReturn) {
        if let Some(expression) = &node.expr {
            let action = if is_err_expression(expression) {
                ErrorAction::Propagate
            } else {
                ErrorAction::Recover
            };
            self.record(action, node.span());
        }
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        let target = expression_leaf(&node.func);
        let action = if target.as_deref() == Some("Err") {
            ErrorAction::Propagate
        } else if target.as_deref().is_some_and(is_recording_target) {
            ErrorAction::Record
        } else {
            ErrorAction::Recover
        };
        self.record(action, node.span());
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let name = node.method.to_string();
        self.record(
            if is_recording_target(&name) {
                ErrorAction::Record
            } else {
                ErrorAction::Recover
            },
            node.span(),
        );
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
        self.record(ErrorAction::Recover, node.span());
        visit::visit_expr_assign(self, node);
    }

    fn visit_expr_break(&mut self, node: &'ast syn::ExprBreak) {
        self.record(ErrorAction::Recover, node.span());
    }

    fn visit_expr_continue(&mut self, node: &'ast syn::ExprContinue) {
        self.record(ErrorAction::Recover, node.span());
    }

    fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
        self.record_macro(&node.mac, node.span());
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        self.record_macro(node, node.span());
        visit::visit_macro(self, node);
    }
}

fn is_err_expression(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::Call(call) => expression_leaf(&call.func).as_deref() == Some("Err"),
        syn::Expr::Paren(value) => is_err_expression(&value.expr),
        syn::Expr::Group(value) => is_err_expression(&value.expr),
        _ => false,
    }
}

fn expression_leaf(expression: &syn::Expr) -> Option<String> {
    match expression {
        syn::Expr::Path(value) => value
            .path
            .segments
            .last()
            .map(|part| part.ident.to_string()),
        _ => None,
    }
}

fn is_recording_target(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "error"
            | "warn"
            | "log"
            | "report_error"
            | "capture_error"
            | "capture_exception"
            | "eprintln"
    )
}

pub(crate) fn actions_for_expr(expression: &syn::Expr) -> Vec<(ErrorAction, Span)> {
    let mut collector = ActionCollector::default();
    collector.visit_expr(expression);
    collector.actions.into_iter().collect()
}

pub(crate) fn actions_for_block(block: &syn::Block) -> Vec<(ErrorAction, Span)> {
    let mut collector = ActionCollector::default();
    collector.visit_block(block);
    collector.actions.into_iter().collect()
}

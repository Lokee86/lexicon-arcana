use rustpython_parser::ast;

use super::super::source::offset;
use super::Visitor;

impl Visitor<'_> {
    pub(super) fn visit_statement(&mut self, statement: &ast::Stmt) {
        match statement {
            ast::Stmt::ClassDef(value) => self.visit_class(value),
            ast::Stmt::FunctionDef(value) => self.visit_function(value),
            ast::Stmt::AsyncFunctionDef(value) => self.visit_async_function(value),
            ast::Stmt::Import(value) => self.visit_import(value),
            ast::Stmt::ImportFrom(value) => self.visit_import_from(value),
            ast::Stmt::Assign(value) => {
                for target in &value.targets {
                    self.record_assignment(target, Some(&value.value), None, value);
                }
                self.visit_expression(&value.value);
            }
            ast::Stmt::AnnAssign(value) => {
                self.record_assignment(
                    &value.target,
                    value.value.as_deref(),
                    Some(&value.annotation),
                    value,
                );
                if let Some(expression) = value.value.as_deref() {
                    self.visit_expression(expression);
                }
            }
            ast::Stmt::AugAssign(value) => {
                self.record_assignment(&value.target, None, None, value);
                self.visit_target(&value.target, true);
                self.visit_expression(&value.value);
            }
            ast::Stmt::Expr(value) => {
                let previous = self.bare_call;
                self.bare_call = match value.value.as_ref() {
                    ast::Expr::Call(call) => Some(offset(call)),
                    _ => None,
                };
                self.visit_expression(&value.value);
                self.bare_call = previous;
            }
            ast::Stmt::Return(value) => {
                if let Some(expression) = value.value.as_deref() {
                    self.visit_expression(expression);
                }
            }
            ast::Stmt::If(value) => {
                self.visit_expression(&value.test);
                self.branch(|this| this.visit_suite(&value.body));
                self.branch(|this| this.visit_suite(&value.orelse));
            }
            ast::Stmt::For(value) => {
                self.visit_expression(&value.iter);
                self.record_loop_targets(&value.target, &value.iter, value);
                self.branch(|this| this.visit_suite(&value.body));
                self.branch(|this| this.visit_suite(&value.orelse));
            }
            ast::Stmt::AsyncFor(value) => {
                self.visit_expression(&value.iter);
                self.record_loop_targets(&value.target, &value.iter, value);
                self.branch(|this| this.visit_suite(&value.body));
                self.branch(|this| this.visit_suite(&value.orelse));
            }
            ast::Stmt::While(value) => {
                self.visit_expression(&value.test);
                self.branch(|this| this.visit_suite(&value.body));
                self.branch(|this| this.visit_suite(&value.orelse));
            }
            ast::Stmt::With(value) => {
                for item in &value.items {
                    self.visit_expression(&item.context_expr);
                    if let Some(target) = item.optional_vars.as_deref() {
                        self.visit_expression(target);
                    }
                }
                self.visit_suite(&value.body);
            }
            ast::Stmt::AsyncWith(value) => {
                for item in &value.items {
                    self.visit_expression(&item.context_expr);
                    if let Some(target) = item.optional_vars.as_deref() {
                        self.visit_expression(target);
                    }
                }
                self.visit_suite(&value.body);
            }
            ast::Stmt::Try(value) => {
                self.visit_try(
                    &value.body,
                    &value.handlers,
                    &value.orelse,
                    &value.finalbody,
                );
            }
            ast::Stmt::TryStar(value) => {
                self.visit_try(
                    &value.body,
                    &value.handlers,
                    &value.orelse,
                    &value.finalbody,
                );
            }
            ast::Stmt::Match(value) => {
                self.visit_expression(&value.subject);
                for case in &value.cases {
                    if let Some(guard) = case.guard.as_deref() {
                        self.visit_expression(guard);
                    }
                    self.branch(|this| this.visit_suite(&case.body));
                }
            }
            ast::Stmt::Raise(value) => {
                if let Some(expression) = value.exc.as_deref() {
                    self.visit_expression(expression);
                }
                if let Some(expression) = value.cause.as_deref() {
                    self.visit_expression(expression);
                }
            }
            ast::Stmt::Assert(value) => {
                self.visit_expression(&value.test);
                if let Some(message) = value.msg.as_deref() {
                    self.visit_expression(message);
                }
            }
            ast::Stmt::Delete(value) => {
                for target in &value.targets {
                    self.visit_expression(target);
                }
            }
            ast::Stmt::TypeAlias(value) => self.visit_expression(&value.value),
            _ => {}
        }
    }

    fn visit_try(
        &mut self,
        body: &ast::Suite,
        handlers: &[ast::ExceptHandler],
        orelse: &ast::Suite,
        finalbody: &ast::Suite,
    ) {
        self.branch(|this| this.visit_suite(body));
        for handler in handlers {
            let ast::ExceptHandler::ExceptHandler(handler) = handler;
            if let Some(kind) = handler.type_.as_deref() {
                self.visit_expression(kind);
            }
            self.branch(|this| this.visit_suite(&handler.body));
        }
        self.branch(|this| this.visit_suite(orelse));
        self.branch(|this| this.visit_suite(finalbody));
    }

    fn branch(&mut self, visit: impl FnOnce(&mut Self)) {
        self.branch_depth += 1;
        visit(self);
        self.branch_depth -= 1;
    }
}

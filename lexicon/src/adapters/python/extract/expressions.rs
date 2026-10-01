use rustpython_parser::ast;

use super::super::model::CallInfo;
use super::super::source::{expression_text, offset, span};
use super::Visitor;

impl Visitor<'_> {
    pub(super) fn visit_expression(&mut self, expression: &ast::Expr) {
        match expression {
            ast::Expr::Name(value) => {
                if value.ctx == ast::ExprContext::Load {
                    self.emit_dataflow(expression, "reads", value.id.as_str());
                }
            }
            ast::Expr::Attribute(value) => {
                self.visit_expression(&value.value);
                if value.ctx == ast::ExprContext::Load
                    && matches!(
                        value.value.as_ref(),
                        ast::Expr::Name(name) if name.id.as_str() == "self"
                    )
                {
                    self.emit_dataflow(expression, "reads", value.attr.as_str());
                }
            }
            ast::Expr::Call(value) => {
                let start = offset(value);
                self.facts.calls.push(CallInfo {
                    module_name: self.file.module.clone(),
                    owner_id: self.owner().to_owned(),
                    class_qname: self.class_qname().map(str::to_owned),
                    scope_id: self.owner().to_owned(),
                    expression_node: value.clone(),
                    callee: (*value.func).clone(),
                    expression: expression_text(expression, &self.file.source),
                    span: span(expression, self.file),
                    bare_expression: self.bare_call == Some(start),
                    outcome_eligible: self.semantic_outcomes_enabled,
                });
                self.visit_expression(&value.func);
                for argument in &value.args {
                    self.visit_expression(argument);
                }
                for keyword in &value.keywords {
                    self.visit_expression(&keyword.value);
                }
            }
            ast::Expr::BoolOp(value) => {
                for item in &value.values {
                    self.visit_expression(item);
                }
            }
            ast::Expr::NamedExpr(value) => {
                self.record_assignment(&value.target, Some(&value.value), None, value);
                self.visit_expression(&value.value);
            }
            ast::Expr::BinOp(value) => {
                self.visit_expression(&value.left);
                self.visit_expression(&value.right);
            }
            ast::Expr::UnaryOp(value) => self.visit_expression(&value.operand),
            ast::Expr::Lambda(value) => self.visit_lambda(value),
            ast::Expr::IfExp(value) => {
                self.visit_expression(&value.test);
                self.visit_expression(&value.body);
                self.visit_expression(&value.orelse);
            }
            ast::Expr::Dict(value) => {
                for key in value.keys.iter().flatten() {
                    self.visit_expression(key);
                }
                for item in &value.values {
                    self.visit_expression(item);
                }
            }
            ast::Expr::Set(value) => self.visit_expressions(&value.elts),
            ast::Expr::List(value) => self.visit_expressions(&value.elts),
            ast::Expr::Tuple(value) => self.visit_expressions(&value.elts),
            ast::Expr::ListComp(value) => {
                self.visit_comprehension(&value.generators);
                self.visit_expression(&value.elt);
            }
            ast::Expr::SetComp(value) => {
                self.visit_comprehension(&value.generators);
                self.visit_expression(&value.elt);
            }
            ast::Expr::GeneratorExp(value) => {
                self.visit_comprehension(&value.generators);
                self.visit_expression(&value.elt);
            }
            ast::Expr::DictComp(value) => {
                self.visit_comprehension(&value.generators);
                self.visit_expression(&value.key);
                self.visit_expression(&value.value);
            }
            ast::Expr::Await(value) => self.visit_expression(&value.value),
            ast::Expr::Yield(value) => {
                if let Some(item) = value.value.as_deref() {
                    self.visit_expression(item);
                }
            }
            ast::Expr::YieldFrom(value) => self.visit_expression(&value.value),
            ast::Expr::Compare(value) => {
                self.visit_expression(&value.left);
                self.visit_expressions(&value.comparators);
            }
            ast::Expr::FormattedValue(value) => {
                self.visit_expression(&value.value);
                if let Some(spec) = value.format_spec.as_deref() {
                    self.visit_expression(spec);
                }
            }
            ast::Expr::JoinedStr(value) => self.visit_expressions(&value.values),
            ast::Expr::Subscript(value) => {
                self.visit_expression(&value.value);
                self.visit_expression(&value.slice);
            }
            ast::Expr::Starred(value) => self.visit_expression(&value.value),
            ast::Expr::Slice(value) => {
                if let Some(item) = value.lower.as_deref() {
                    self.visit_expression(item);
                }
                if let Some(item) = value.upper.as_deref() {
                    self.visit_expression(item);
                }
                if let Some(item) = value.step.as_deref() {
                    self.visit_expression(item);
                }
            }
            _ => {}
        }
    }

    pub(super) fn visit_target(&mut self, target: &ast::Expr, compound: bool) {
        match target {
            ast::Expr::Name(value) => {
                if compound {
                    self.emit_dataflow(target, "reads", value.id.as_str());
                }
                self.emit_dataflow(target, "writes", value.id.as_str());
            }
            ast::Expr::Attribute(value) => {
                self.visit_expression(&value.value);
                if matches!(
                    value.value.as_ref(),
                    ast::Expr::Name(name) if name.id.as_str() == "self"
                ) {
                    if compound {
                        self.emit_dataflow(target, "reads", value.attr.as_str());
                    }
                    self.emit_dataflow(target, "writes", value.attr.as_str());
                }
            }
            ast::Expr::Tuple(value) => {
                for item in &value.elts {
                    self.visit_target(item, compound);
                }
            }
            ast::Expr::List(value) => {
                for item in &value.elts {
                    self.visit_target(item, compound);
                }
            }
            _ => self.visit_expression(target),
        }
    }

    fn visit_comprehension(&mut self, generators: &[ast::Comprehension]) {
        for generator in generators {
            self.visit_expression(&generator.iter);
            self.record_loop_targets(&generator.target, &generator.iter, &generator.iter);
            for condition in &generator.ifs {
                self.visit_expression(condition);
            }
        }
    }

    fn visit_expressions(&mut self, values: &[ast::Expr]) {
        for value in values {
            self.visit_expression(value);
        }
    }
}

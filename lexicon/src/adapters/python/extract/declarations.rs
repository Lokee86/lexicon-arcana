use rustpython_parser::ast;
use serde_json::json;

use crate::SourceSpan;

use super::super::model::{ClassInfo, FunctionInfo, InheritanceInfo};
use super::super::source::{byte_location, expression_text, span};
use super::Visitor;
use super::dataflow::target_bindings;

impl Visitor<'_> {
    pub(super) fn visit_suite(&mut self, suite: &ast::Suite) {
        for statement in suite {
            self.visit_statement(statement);
        }
    }

    pub(super) fn visit_class(&mut self, value: &ast::StmtClassDef) {
        let name = value.name.to_string();
        let qname = self.qualified(&name);
        let kind = if self.is_contract(value) {
            "interface"
        } else {
            "type"
        };
        let record_span = self.node_span(value);
        let mut attributes = self.attributes(&value.decorator_list, false);
        if !value.bases.is_empty() {
            let mut bases = value
                .bases
                .iter()
                .map(|base| expression_text(base, &self.file.source))
                .collect::<Vec<_>>();
            bases.sort();
            merge_attribute(&mut attributes, "bases", json!(bases));
        }
        let id = self.facts.add_node(
            kind,
            &name,
            &self.file.relative,
            &qname,
            None,
            record_span.clone(),
            attributes,
            None,
        );
        self.facts.symbols.insert(qname.clone(), id.clone());
        let owner = self.owner().to_owned();
        self.facts.scope_parents.insert(id.clone(), owner.clone());
        self.facts.classes.insert(
            qname.clone(),
            ClassInfo {
                module_name: self.file.module.clone(),
                bases: value.bases.clone(),
            },
        );
        self.facts
            .add_edge(&owner, &id, "defines", record_span, None);

        for base in &value.bases {
            self.facts.inheritances.push(InheritanceInfo {
                source_id: id.clone(),
                module_name: self.file.module.clone(),
                class_qname: qname.clone(),
                base: base.clone(),
                expression: expression_text(base, &self.file.source),
                span: span(base, &self.file.relative, &self.file.source),
            });
        }

        self.classes.push(qname);
        self.lexical.push((name, true));
        self.owner.push(id);
        for statement in &value.body {
            let previous = self.direct_class_statement;
            self.direct_class_statement =
                matches!(statement, ast::Stmt::Assign(_) | ast::Stmt::AnnAssign(_));
            self.visit_statement(statement);
            self.direct_class_statement = previous;
        }
        self.owner.pop();
        self.lexical.pop();
        self.classes.pop();
    }

    pub(super) fn visit_function(&mut self, value: &ast::StmtFunctionDef) {
        self.visit_function_parts(
            value.name.as_str(),
            &value.args,
            &value.body,
            &value.decorator_list,
            value.returns.as_deref(),
            false,
            self.node_span(value),
        );
    }

    pub(super) fn visit_async_function(&mut self, value: &ast::StmtAsyncFunctionDef) {
        self.visit_function_parts(
            value.name.as_str(),
            &value.args,
            &value.body,
            &value.decorator_list,
            value.returns.as_deref(),
            true,
            self.node_span(value),
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn visit_function_parts(
        &mut self,
        name: &str,
        args: &ast::Arguments,
        body: &ast::Suite,
        decorators: &[ast::Expr],
        returns: Option<&ast::Expr>,
        is_async: bool,
        record_span: Option<SourceSpan>,
    ) {
        let qname = self.qualified(name);
        let kind = if self.lexical.last().is_some_and(|(_, is_class)| *is_class) {
            "method"
        } else {
            "function"
        };
        let id = self.facts.add_node(
            kind,
            name,
            &self.file.relative,
            &qname,
            None,
            record_span.clone(),
            self.attributes(decorators, is_async),
            None,
        );
        self.facts.symbols.insert(qname.clone(), id.clone());
        let owner = self.owner().to_owned();
        self.facts.scope_parents.insert(id.clone(), owner.clone());
        self.facts
            .add_edge(&owner, &id, "defines", record_span, None);

        let parameters = parameters(args);
        self.facts.functions.insert(
            id.clone(),
            FunctionInfo {
                module_name: self.file.module.clone(),
                qname: qname.clone(),
                node_id: id.clone(),
                class_qname: self.class_qname().map(str::to_owned),
                arguments: args.clone(),
                decorators: decorators.to_vec(),
                return_expressions: return_expressions(body),
                parameters: parameters.clone(),
                return_annotation: returns.cloned(),
                is_lambda: false,
                is_async,
            },
        );

        self.lexical.push((name.to_owned(), false));
        self.owner.push(id);
        let previous_depth = self.branch_depth;
        self.branch_depth = 0;
        for (parameter, _) in &parameters {
            self.declare_data(parameter, "parameter", None);
        }
        self.predeclare_locals(body);
        self.visit_suite(body);
        self.branch_depth = previous_depth;
        self.owner.pop();
        self.lexical.pop();
    }

    pub(super) fn visit_lambda(&mut self, value: &ast::ExprLambda) {
        let (line, column) = byte_location(value, &self.file.source);
        let name = format!("<lambda>@{line}:{}", column + 1);
        let owner_qname = self
            .facts
            .qnames
            .get(self.owner())
            .cloned()
            .unwrap_or_else(|| self.file.module.clone());
        let qname = format!("{owner_qname}.{name}");
        let record_span = self.node_span(value);
        let id = self.facts.add_node(
            "function",
            &name,
            &self.file.relative,
            &qname,
            None,
            record_span.clone(),
            Some(json!({"lambda": true})),
            None,
        );
        let parameters = parameters(&value.args);
        self.facts.functions.insert(
            id.clone(),
            FunctionInfo {
                module_name: self.file.module.clone(),
                qname: qname.clone(),
                node_id: id.clone(),
                class_qname: self.class_qname().map(str::to_owned),
                arguments: (*value.args).clone(),
                decorators: Vec::new(),
                return_expressions: vec![(*value.body).clone()],
                parameters: parameters.clone(),
                return_annotation: None,
                is_lambda: true,
                is_async: false,
            },
        );
        let owner = self.owner().to_owned();
        self.facts.scope_parents.insert(id.clone(), owner.clone());
        self.facts.lambda_ids.insert(
            (
                self.file.module.clone(),
                super::super::source::offset(value),
            ),
            id.clone(),
        );
        self.facts
            .add_edge(&owner, &id, "defines", record_span, None);

        self.lexical.push((name, false));
        self.owner.push(id);
        for (parameter, _) in &parameters {
            self.declare_data(parameter, "parameter", None);
        }
        self.visit_expression(&value.body);
        self.owner.pop();
        self.lexical.pop();
    }

    pub(super) fn qualified(&self, leaf: &str) -> String {
        let mut parts = vec![self.file.module.clone()];
        parts.extend(self.lexical.iter().map(|(name, _)| name.clone()));
        parts.push(leaf.to_owned());
        parts.join(".")
    }

    fn is_contract(&self, value: &ast::StmtClassDef) -> bool {
        value.bases.iter().any(|base| {
            expression_text(base, &self.file.source)
                .rsplit('.')
                .next()
                .is_some_and(|name| matches!(name, "Protocol" | "ABC" | "Interface" | "Trait"))
        }) || value.decorator_list.iter().any(|decorator| {
            expression_text(decorator, &self.file.source)
                .rsplit('.')
                .next()
                .is_some_and(|name| matches!(name, "runtime_checkable" | "abstractclass"))
        })
    }

    fn predeclare_locals(&mut self, body: &ast::Suite) {
        for statement in body {
            self.predeclare_statement(statement);
        }
    }

    fn predeclare_statement(&mut self, statement: &ast::Stmt) {
        match statement {
            ast::Stmt::FunctionDef(_) | ast::Stmt::AsyncFunctionDef(_) | ast::Stmt::ClassDef(_) => {
            }
            ast::Stmt::Assign(value) => {
                for target in &value.targets {
                    self.predeclare_target(target);
                }
            }
            ast::Stmt::AnnAssign(value) => self.predeclare_target(&value.target),
            ast::Stmt::AugAssign(value) => self.predeclare_target(&value.target),
            ast::Stmt::For(value) => {
                self.predeclare_target(&value.target);
                self.predeclare_locals(&value.body);
                self.predeclare_locals(&value.orelse);
            }
            ast::Stmt::AsyncFor(value) => {
                self.predeclare_target(&value.target);
                self.predeclare_locals(&value.body);
                self.predeclare_locals(&value.orelse);
            }
            ast::Stmt::If(value) => {
                self.predeclare_locals(&value.body);
                self.predeclare_locals(&value.orelse);
            }
            ast::Stmt::While(value) => {
                self.predeclare_locals(&value.body);
                self.predeclare_locals(&value.orelse);
            }
            ast::Stmt::With(value) => self.predeclare_locals(&value.body),
            ast::Stmt::AsyncWith(value) => self.predeclare_locals(&value.body),
            ast::Stmt::Try(value) => {
                self.predeclare_locals(&value.body);
                for handler in &value.handlers {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    self.predeclare_locals(&handler.body);
                }
                self.predeclare_locals(&value.orelse);
                self.predeclare_locals(&value.finalbody);
            }
            ast::Stmt::TryStar(value) => {
                self.predeclare_locals(&value.body);
                for handler in &value.handlers {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    self.predeclare_locals(&handler.body);
                }
                self.predeclare_locals(&value.orelse);
                self.predeclare_locals(&value.finalbody);
            }
            _ => {}
        }
    }

    fn predeclare_target(&mut self, target: &ast::Expr) {
        for (name, _) in target_bindings(target) {
            self.declare_data(&name, "variable", self.node_span(target));
        }
    }
}

fn parameters(args: &ast::Arguments) -> Vec<(String, Option<ast::Expr>)> {
    let mut values = Vec::new();
    for value in args
        .posonlyargs
        .iter()
        .chain(args.args.iter())
        .chain(args.kwonlyargs.iter())
    {
        values.push((
            value.def.arg.to_string(),
            value.def.annotation.as_deref().cloned(),
        ));
    }
    if let Some(value) = &args.vararg {
        values.push((value.arg.to_string(), value.annotation.as_deref().cloned()));
    }
    if let Some(value) = &args.kwarg {
        values.push((value.arg.to_string(), value.annotation.as_deref().cloned()));
    }
    values
}

fn return_expressions(body: &ast::Suite) -> Vec<ast::Expr> {
    let mut values = Vec::new();
    collect_returns(body, &mut values);
    values
}

fn collect_returns(body: &ast::Suite, values: &mut Vec<ast::Expr>) {
    for statement in body {
        match statement {
            ast::Stmt::Return(value) => {
                if let Some(value) = value.value.as_deref() {
                    values.push(value.clone());
                }
            }
            ast::Stmt::FunctionDef(_) | ast::Stmt::AsyncFunctionDef(_) | ast::Stmt::ClassDef(_) => {
            }
            ast::Stmt::If(value) => {
                collect_returns(&value.body, values);
                collect_returns(&value.orelse, values);
            }
            ast::Stmt::For(value) => {
                collect_returns(&value.body, values);
                collect_returns(&value.orelse, values);
            }
            ast::Stmt::AsyncFor(value) => {
                collect_returns(&value.body, values);
                collect_returns(&value.orelse, values);
            }
            ast::Stmt::While(value) => {
                collect_returns(&value.body, values);
                collect_returns(&value.orelse, values);
            }
            ast::Stmt::With(value) => collect_returns(&value.body, values),
            ast::Stmt::AsyncWith(value) => collect_returns(&value.body, values),
            ast::Stmt::Try(value) => {
                collect_returns(&value.body, values);
                for handler in &value.handlers {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    collect_returns(&handler.body, values);
                }
                collect_returns(&value.orelse, values);
                collect_returns(&value.finalbody, values);
            }
            ast::Stmt::TryStar(value) => {
                collect_returns(&value.body, values);
                for handler in &value.handlers {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    collect_returns(&handler.body, values);
                }
                collect_returns(&value.orelse, values);
                collect_returns(&value.finalbody, values);
            }
            _ => {}
        }
    }
}

fn merge_attribute(target: &mut Option<serde_json::Value>, key: &str, value: serde_json::Value) {
    if target.is_none() {
        *target = Some(json!({}));
    }
    if let Some(serde_json::Value::Object(map)) = target {
        map.insert(key.to_owned(), value);
    }
}

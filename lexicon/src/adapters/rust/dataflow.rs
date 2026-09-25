use crate::adapters::rust::model::{Context, FunctionBody, FunctionInfo};
use crate::adapters::rust::paths::span_value;
use crate::adapters::rust::resolve;
use quote::ToTokens;
use std::collections::BTreeMap;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

pub(crate) fn emit(context: &mut Context) {
    let functions: Vec<FunctionInfo> = context.functions.values().cloned().collect();
    for function in functions {
        emit_function(context, &function);
    }
}

fn emit_function(context: &mut Context, function: &FunctionInfo) {
    let mut symbols = BTreeMap::new();
    for parameter in &function.parameters {
        if parameter.name == "_" {
            continue;
        }
        let qn = format!("{}::parameter::{}", function.qn, parameter.name);
        let id = context.facts.add_node(
            "rust",
            "parameter",
            &qn,
            &parameter.name,
            &function.source_path,
            &qn,
            None,
            None,
            BTreeMap::new(),
        );
        context.facts.add_edge(&function.id, &id, "defines", None);
        symbols.insert(parameter.name.clone(), id);
    }
    for (qualified_name, id) in &context.symbols {
        if context.value_types.contains_key(qualified_name)
            && qualified_name.starts_with(&format!("{}::", function.module_qn))
            && let Some(name) = qualified_name.rsplit("::").next()
        {
            symbols
                .entry(name.to_string())
                .or_insert_with(|| id.clone());
        }
    }
    let self_type = function.self_type.as_deref().and_then(|text| {
        resolve::resolve_type_ids(context, text, function)
            .into_iter()
            .next()
    });
    let mut visitor = DataflowVisitor {
        context,
        function,
        scopes: vec![symbols],
        self_type,
    };
    match &function.body {
        FunctionBody::Block(block) => visitor.visit_block(block),
        FunctionBody::Expr(expression) => visitor.visit_expr(expression),
    }
}

struct DataflowVisitor<'a> {
    context: &'a mut Context,
    function: &'a FunctionInfo,
    scopes: Vec<BTreeMap<String, String>>,
    self_type: Option<String>,
}

impl DataflowVisitor<'_> {
    fn span(&self, node: impl Spanned) -> Option<serde_json::Value> {
        span_value(node.span(), &self.function.source_path)
    }

    fn resolve(&self, name: &str) -> Option<String> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn read(&mut self, id: &str, node: impl Spanned) {
        self.context
            .facts
            .add_dataflow_edge(&self.function.id, id, "reads", self.span(node));
    }

    fn write(&mut self, id: &str, node: impl Spanned) {
        self.context
            .facts
            .add_dataflow_edge(&self.function.id, id, "writes", self.span(node));
    }

    fn bind_pattern(&mut self, pattern: &syn::Pat) {
        match pattern {
            syn::Pat::Ident(value) => {
                let name = value.ident.to_string();
                if name == "_" {
                    return;
                }
                let qn = format!(
                    "{}::local::{}::{}",
                    self.function.qn,
                    self.scopes.len(),
                    name
                );
                let id = self.context.facts.add_node(
                    "rust",
                    "variable",
                    &qn,
                    &name,
                    &self.function.source_path,
                    &qn,
                    None,
                    self.span(pattern),
                    BTreeMap::new(),
                );
                self.context
                    .facts
                    .add_edge(&self.function.id, &id, "defines", self.span(pattern));
                self.scopes.last_mut().unwrap().insert(name, id.clone());
                self.write(&id, pattern);
                if let Some((_, subpattern)) = &value.subpat {
                    self.bind_pattern(subpattern);
                }
            }
            syn::Pat::Type(value) => self.bind_pattern(&value.pat),
            syn::Pat::Reference(value) => self.bind_pattern(&value.pat),
            syn::Pat::Tuple(value) => {
                for item in &value.elems {
                    self.bind_pattern(item);
                }
            }
            syn::Pat::Struct(value) => {
                for field in &value.fields {
                    self.bind_pattern(&field.pat);
                }
            }
            syn::Pat::Slice(value) => {
                for item in &value.elems {
                    self.bind_pattern(item);
                }
            }
            _ => {}
        }
    }

    fn field_type_qn(&self, field: &syn::ExprField) -> Option<String> {
        let type_id = match field.base.as_ref() {
            syn::Expr::Path(path) if path.path.segments.len() == 1 => {
                let name = path.path.segments[0].ident.to_string();
                if name == "self" {
                    self.self_type.clone()
                } else {
                    self.function
                        .parameters
                        .iter()
                        .find(|parameter| parameter.name == name)
                        .and_then(|parameter| parameter.type_text.as_deref())
                        .and_then(|type_text| {
                            resolve::resolve_type_ids(self.context, type_text, self.function)
                                .into_iter()
                                .next()
                        })
                }
            }
            _ => None,
        }?;
        self.context.type_qn_by_id.get(&type_id).cloned()
    }

    fn field_edge(&mut self, field: &syn::ExprField, relation: &str) {
        let Some(type_qn) = self.field_type_qn(field) else {
            return;
        };
        let name = match &field.member {
            syn::Member::Named(name) => name.to_string(),
            syn::Member::Unnamed(index) => index.index.to_string(),
        };
        if let Some(id) = self.context.field_ids.get(&(type_qn, name)).cloned() {
            self.context.facts.add_dataflow_edge(
                &self.function.id,
                &id,
                relation,
                self.span(field),
            );
        }
    }

    fn target(&mut self, expression: &syn::Expr, compound: bool) {
        match expression {
            syn::Expr::Path(path) if path.path.segments.len() == 1 => {
                let name = path.path.segments[0].ident.to_string();
                if let Some(id) = self.resolve(&name) {
                    if compound {
                        self.read(&id, path);
                    }
                    self.write(&id, path);
                }
            }
            syn::Expr::Field(field) => {
                self.visit_expr(&field.base);
                if compound {
                    self.field_edge(field, "reads");
                }
                self.field_edge(field, "writes");
            }
            _ => self.visit_expr(expression),
        }
    }
}

impl<'ast> Visit<'ast> for DataflowVisitor<'_> {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.scopes.push(BTreeMap::new());
        for statement in &block.stmts {
            self.visit_stmt(statement);
        }
        self.scopes.pop();
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let Some(init) = &local.init {
            self.visit_expr(&init.expr);
            if let Some((_, diverge)) = &init.diverge {
                self.visit_expr(diverge);
            }
        }
        self.bind_pattern(&local.pat);
    }

    fn visit_expr(&mut self, expression: &'ast syn::Expr) {
        match expression {
            syn::Expr::Assign(value) => {
                self.visit_expr(&value.right);
                self.target(&value.left, false);
            }
            syn::Expr::Binary(value) if is_compound_assign(value) => {
                self.target(&value.left, true);
                self.visit_expr(&value.right);
            }
            syn::Expr::Field(value) => {
                self.visit_expr(&value.base);
                self.field_edge(value, "reads");
            }
            syn::Expr::Path(value) if value.path.segments.len() == 1 => {
                let name = value.path.segments[0].ident.to_string();
                if let Some(id) = self.resolve(&name) {
                    self.read(&id, value);
                }
            }
            _ => visit::visit_expr(self, expression),
        }
    }
}

fn is_compound_assign(value: &syn::ExprBinary) -> bool {
    let operator = value.op.to_token_stream().to_string();
    operator.ends_with('=') && operator != "=="
}

use crate::adapters::rust::call_model::AnalysisResult;
use crate::adapters::rust::model::{Context, FunctionBody, FunctionInfo, ValueSet};
use crate::adapters::rust::resolve;
use std::collections::BTreeMap;
use syn::{Pat, Stmt};

pub(crate) struct Analyzer<'a> {
    pub(crate) context: &'a Context,
    pub(crate) function: &'a FunctionInfo,
    pub(crate) env: BTreeMap<String, ValueSet>,
    pub(crate) result: AnalysisResult,
}

impl<'a> Analyzer<'a> {
    pub(crate) fn new(context: &'a Context, function: &'a FunctionInfo) -> Self {
        let parameter_names: std::collections::BTreeSet<_> = function
            .parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect();
        let mut env = BTreeMap::<String, ValueSet>::new();
        for ((closure_id, name), value) in &context.propagated_captures {
            if closure_id == &function.id && !parameter_names.contains(name.as_str()) {
                env.entry(name.clone()).or_default().merge(value);
            }
        }
        for (index, parameter) in function.parameters.iter().enumerate() {
            let mut value = parameter
                .type_text
                .as_deref()
                .map(|text| resolve::value_from_type(context, text, function))
                .unwrap_or_default();
            if parameter.callable_bound {
                value.dynamic_callable = true;
                value.unknown = true;
            }
            if let Some(propagated) = context
                .propagated_parameters
                .get(&(function.id.clone(), index))
            {
                value.merge(propagated);
            }
            env.insert(parameter.name.clone(), value);
        }
        if let Some(self_type) = &function.self_type {
            let value = resolve::value_from_type(context, self_type, function);
            env.entry("self".into()).or_default().merge(&value);
        }
        let mut result = AnalysisResult::default();
        if let Some(return_type) = &function.return_type {
            result
                .return_value
                .merge(&resolve::value_from_type(context, return_type, function));
        }
        Self {
            context,
            function,
            env,
            result,
        }
    }

    pub(crate) fn run(mut self) -> AnalysisResult {
        match &self.function.body {
            FunctionBody::Block(block) => {
                let value = self.eval_block(block);
                self.result.return_value.merge(&value);
            }
            FunctionBody::Expr(expression) => {
                let value = self.eval_expr(expression);
                self.result.return_value.merge(&value);
            }
        }
        self.result
    }

    pub(crate) fn eval_block(&mut self, block: &syn::Block) -> ValueSet {
        let mut last = ValueSet::default();
        for statement in &block.stmts {
            match statement {
                Stmt::Local(local) => self.eval_local(local),
                Stmt::Expr(expression, semi) => {
                    let value = self.eval_expr(expression);
                    if semi.is_none() {
                        last = value;
                    }
                }
                Stmt::Macro(value) => {
                    crate::adapters::rust::expr_eval::evaluate_statement_macro(self, value)
                }
                Stmt::Item(_) => {}
            }
        }
        last
    }

    fn eval_local(&mut self, local: &syn::Local) {
        let mut value = local
            .init
            .as_ref()
            .map(|init| self.eval_expr(&init.expr))
            .unwrap_or_default();
        if let Pat::Type(typed) = &local.pat {
            value.merge(&resolve::value_from_type(
                self.context,
                &crate::adapters::rust::syntax::type_tokens(&typed.ty),
                self.function,
            ));
        }
        self.bind_pattern(&local.pat, &value);
        if let Some(init) = &local.init
            && let Some((_, diverge)) = &init.diverge
        {
            self.eval_expr(diverge);
        }
    }

    pub(crate) fn bind_pattern(&mut self, pattern: &Pat, value: &ValueSet) {
        match pattern {
            Pat::Ident(identifier) => {
                self.env
                    .entry(identifier.ident.to_string())
                    .or_default()
                    .merge(value);
                if let Some((_, sub)) = &identifier.subpat {
                    self.bind_pattern(sub, value);
                }
            }
            Pat::Type(typed) => {
                let mut typed_value = value.clone();
                typed_value.merge(&resolve::value_from_type(
                    self.context,
                    &crate::adapters::rust::syntax::type_tokens(&typed.ty),
                    self.function,
                ));
                self.bind_pattern(&typed.pat, &typed_value);
            }
            Pat::Reference(reference) => self.bind_pattern(&reference.pat, value),
            Pat::Tuple(tuple) => {
                let elements = if value.tuple_elements.len() == tuple.elems.len() {
                    Some(value.tuple_elements.as_slice())
                } else if value.contained_values.len() == tuple.elems.len() {
                    Some(value.contained_values.as_slice())
                } else {
                    value
                        .contained_values
                        .first()
                        .filter(|inner| inner.tuple_elements.len() == tuple.elems.len())
                        .map(|inner| inner.tuple_elements.as_slice())
                };
                if let Some(elements) = elements {
                    for (element, element_value) in tuple.elems.iter().zip(elements) {
                        self.bind_pattern(element, element_value);
                    }
                } else {
                    for element in &tuple.elems {
                        self.bind_pattern(element, value);
                    }
                }
            }
            Pat::TupleStruct(tuple) => {
                let inner = value.contained_values.first().unwrap_or(value);
                if inner.tuple_elements.len() == tuple.elems.len() {
                    for (element, element_value) in tuple.elems.iter().zip(&inner.tuple_elements) {
                        self.bind_pattern(element, element_value);
                    }
                } else {
                    for element in &tuple.elems {
                        self.bind_pattern(element, inner);
                    }
                }
            }
            Pat::Struct(structure) => {
                for field in &structure.fields {
                    self.bind_pattern(&field.pat, value);
                }
            }
            Pat::Slice(slice) => {
                for element in &slice.elems {
                    self.bind_pattern(element, value);
                }
            }
            Pat::Or(or_pattern) => {
                for case in &or_pattern.cases {
                    self.bind_pattern(case, value);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn assign_name(&mut self, name: &str, value: &ValueSet) {
        self.env.entry(name.into()).or_default().merge(value);
    }

    pub(crate) fn eval_expr(&mut self, expression: &syn::Expr) -> ValueSet {
        crate::adapters::rust::expr_eval::evaluate(self, expression)
    }
}

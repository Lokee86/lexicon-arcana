use crate::adapters::rust::model::{Context, FunctionBody, FunctionInfo};
use crate::adapters::rust::paths::{span_start, span_value};
use crate::adapters::rust::syntax;
use serde_json::Value;
use std::collections::BTreeMap;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

pub(crate) struct Registration<'a> {
    pub(crate) id: String,
    pub(crate) qn: String,
    pub(crate) module_qn: &'a str,
    pub(crate) crate_qn: &'a str,
    pub(crate) source_path: &'a str,
    pub(crate) signature: &'a syn::Signature,
    pub(crate) body: FunctionBody,
    pub(crate) self_type: Option<String>,
    pub(crate) trait_path: Option<String>,
}

pub(crate) fn register(context: &mut Context, registration: Registration<'_>) {
    let info = FunctionInfo {
        id: registration.id.clone(),
        qn: registration.qn.clone(),
        module_qn: registration.module_qn.into(),
        crate_qn: registration.crate_qn.into(),
        source_path: registration.source_path.into(),
        body: registration.body.clone(),
        parameters: syntax::signature_parameters(registration.signature),
        return_type: syntax::return_type(&registration.signature.output),
        self_type: registration.self_type,
        trait_path: registration.trait_path,
        generic_bounds: syntax::generic_bounds(registration.signature),
    };
    context
        .function_qn_by_id
        .insert(registration.id.clone(), registration.qn);
    context
        .functions
        .insert(registration.id.clone(), info.clone());
    register_closures(context, &info);
}

fn register_closures(context: &mut Context, parent: &FunctionInfo) {
    let mut collector = ClosureCollector {
        closures: Vec::new(),
    };
    match &parent.body {
        FunctionBody::Block(block) => collector.visit_block(block),
        FunctionBody::Expr(expression) => collector.visit_expr(expression),
    }
    for closure in collector.closures {
        let start = span_start(closure.span());
        let key = (parent.source_path.clone(), start.0, start.1);
        if context.closure_ids.contains_key(&key) {
            continue;
        }
        let name = format!("closure@{}:{}", start.0, start.1);
        let qn = format!("{}::{name}", parent.qn);
        let mut attributes = BTreeMap::new();
        attributes.insert("language_kind".into(), Value::String("closure".into()));
        let id = context.facts.add_node(
            "rust",
            "function",
            &qn,
            &name,
            &parent.source_path,
            &qn,
            None,
            span_value(closure.span(), &parent.source_path),
            attributes,
        );
        crate::adapters::rust::relationships::define_and_contain(
            context,
            &parent.id,
            &id,
            closure.span(),
            &parent.source_path,
        );
        context.closure_ids.insert(key, id.clone());
        context.function_qn_by_id.insert(id.clone(), qn.clone());
        context.functions.insert(
            id.clone(),
            FunctionInfo {
                id,
                qn,
                module_qn: parent.module_qn.clone(),
                crate_qn: parent.crate_qn.clone(),
                source_path: parent.source_path.clone(),
                body: FunctionBody::Expr((*closure.body).clone()),
                parameters: syntax::closure_parameters(&closure),
                return_type: syntax::return_type(&closure.output),
                self_type: parent.self_type.clone(),
                trait_path: parent.trait_path.clone(),
                generic_bounds: parent.generic_bounds.clone(),
            },
        );
    }
}

struct ClosureCollector {
    closures: Vec<syn::ExprClosure>,
}
impl<'ast> Visit<'ast> for ClosureCollector {
    fn visit_expr_closure(&mut self, value: &'ast syn::ExprClosure) {
        self.closures.push(value.clone());
        visit::visit_expr_closure(self, value);
    }
}

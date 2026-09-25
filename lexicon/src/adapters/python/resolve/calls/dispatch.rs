use std::collections::BTreeSet;

use rustpython_parser::ast;

use super::super::super::source::dotted;
use super::super::relationships::{descendants, mro_qnames};
use super::super::shapes::TypeShape;
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn method_targets(&mut self, class_id: &str, method: &str) -> BTreeSet<String> {
        let Some(class) = self.facts.qnames.get(class_id) else {
            return BTreeSet::new();
        };
        if matches!(self.kind(class_id), Some("interface" | "trait")) {
            let mut result = BTreeSet::new();
            for descendant in descendants(self.facts, class) {
                if let Some(id) = self.facts.symbols.get(&descendant)
                    && !matches!(self.kind(id), Some("interface" | "trait"))
                {
                    result.extend(self.method_targets(id, method));
                }
            }
            return result;
        }

        for candidate in mro_qnames(self.facts, class) {
            if let Some(target) = self.facts.symbols.get(&format!("{candidate}.{method}"))
                && self.kind(target) == Some("method")
            {
                return BTreeSet::from([target.clone()]);
            }
        }
        BTreeSet::new()
    }

    pub(super) fn super_method_targets(
        &self,
        class_qname: Option<&str>,
        method: &str,
    ) -> BTreeSet<String> {
        let Some(class_qname) = class_qname else {
            return BTreeSet::new();
        };
        for candidate in mro_qnames(self.facts, class_qname).into_iter().skip(1) {
            if let Some(target) = self.facts.symbols.get(&format!("{candidate}.{method}"))
                && self.kind(target) == Some("method")
            {
                return BTreeSet::from([target.clone()]);
            }
        }
        BTreeSet::new()
    }

    pub(super) fn callable_targets(
        &mut self,
        callee: &ast::Expr,
        module: &str,
        class_qname: Option<&str>,
        scope: Option<&str>,
        before: u32,
        seen: &mut BTreeSet<(String, String)>,
    ) -> (BTreeSet<String>, String) {
        match callee {
            ast::Expr::Name(value) => {
                if value.id.as_str() == "cls"
                    && let Some(class_qname) = class_qname
                    && let Some(id) = self.facts.symbols.get(class_qname)
                {
                    return (BTreeSet::from([id.clone()]), String::new());
                }

                let local =
                    self.local_shape(value.id.as_str(), module, class_qname, scope, before, seen);
                if !local.callables.is_empty() {
                    return (local.callables, String::new());
                }
                let mut callable_instances = BTreeSet::new();
                for class_id in &local.direct {
                    callable_instances.extend(self.method_targets(class_id, "__call__"));
                }
                if !callable_instances.is_empty() {
                    return (callable_instances, String::new());
                }
                if let Some(reason) = local.reason() {
                    return (BTreeSet::new(), reason);
                }

                let (target, reason) =
                    self.reference(module, class_qname, Some(value.id.as_str()), scope);
                let Some(target) = target else {
                    return (BTreeSet::new(), reason);
                };
                match self.kind(&target) {
                    Some("function" | "method") => {
                        (self.effective_target_ids(&target), String::new())
                    }
                    Some("type") => (BTreeSet::from([target]), String::new()),
                    _ => (BTreeSet::new(), "dynamic-target".into()),
                }
            }
            ast::Expr::Attribute(value) => {
                if let ast::Expr::Call(call) = value.value.as_ref()
                    && dotted(&call.func).as_deref() == Some("super")
                {
                    let targets = self.super_method_targets(class_qname, value.attr.as_str());
                    return if targets.is_empty() {
                        (targets, "missing-target".into())
                    } else {
                        (targets, String::new())
                    };
                }

                let reference = dotted(callee);
                let (direct, direct_reason) =
                    self.reference(module, class_qname, reference.as_deref(), scope);
                if let Some(target) = direct {
                    match self.kind(&target) {
                        Some("function" | "method") => {
                            return (self.effective_target_ids(&target), String::new());
                        }
                        Some("type") => {
                            return (BTreeSet::from([target]), String::new());
                        }
                        _ => {}
                    }
                }

                let receiver =
                    self.expression_shape(&value.value, module, class_qname, scope, before, seen);
                let mut targets = BTreeSet::new();
                for class_id in &receiver.direct {
                    targets.extend(self.method_targets(class_id, value.attr.as_str()));
                }
                if !targets.is_empty() {
                    return (targets, String::new());
                }
                if let Some(reason) = receiver.reason() {
                    return (BTreeSet::new(), reason);
                }
                if !receiver.direct.is_empty() {
                    return (BTreeSet::new(), "missing-target".into());
                }
                (
                    BTreeSet::new(),
                    if direct_reason == "missing-target" {
                        "dynamic-target".into()
                    } else {
                        direct_reason
                    },
                )
            }
            ast::Expr::Lambda(value) => {
                let key = (
                    module.to_owned(),
                    super::super::super::source::offset(value),
                );
                self.facts
                    .lambda_ids
                    .get(&key)
                    .cloned()
                    .map_or((BTreeSet::new(), "dynamic-target".into()), |id| {
                        (BTreeSet::from([id]), String::new())
                    })
            }
            ast::Expr::Call(_) | ast::Expr::Subscript(_) | ast::Expr::IfExp(_) => {
                let shape = self.expression_shape(callee, module, class_qname, scope, before, seen);
                let reason = shape.reason();
                let mut targets = shape.callables.clone();
                for class_id in &shape.direct {
                    targets.extend(self.method_targets(class_id, "__call__"));
                }
                if targets.is_empty() {
                    (targets, reason.unwrap_or_else(|| "dynamic-target".into()))
                } else {
                    (targets, String::new())
                }
            }
            _ => (BTreeSet::new(), "dynamic-target".into()),
        }
    }

    pub(super) fn annotation_reference_shape(&mut self, id: Option<String>) -> TypeShape {
        let Some(id) = id else {
            return TypeShape::default();
        };
        if !matches!(self.kind(&id), Some("type" | "interface" | "trait")) {
            return TypeShape::default();
        }
        let mut shape = TypeShape::direct(id.clone());
        if let Some(qname) = self.facts.qnames.get(&id) {
            for descendant in descendants(self.facts, qname) {
                if let Some(descendant_id) = self.facts.symbols.get(&descendant) {
                    shape.direct.insert(descendant_id.clone());
                }
            }
        }
        shape
    }
    pub(super) fn base_qnames(&mut self, class_qname: &str) -> Vec<String> {
        if let Some(value) = self.base_cache.get(class_qname) {
            return value.clone();
        }
        let value = super::super::relationships::base_qnames(self.facts, class_qname);
        self.base_cache
            .insert(class_qname.to_owned(), value.clone());
        value
    }

    pub(super) fn instance_type_ids(&mut self, class_id: &str) -> BTreeSet<String> {
        let Some(qname) = self.facts.qnames.get(class_id).cloned() else {
            return BTreeSet::from([class_id.to_owned()]);
        };
        let mut ids = BTreeSet::from([class_id.to_owned()]);
        for descendant in descendants(self.facts, &qname) {
            if let Some(id) = self.facts.symbols.get(&descendant) {
                ids.insert(id.clone());
            }
        }
        ids
    }
}

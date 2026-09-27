use std::collections::{BTreeMap, BTreeSet};

use rustpython_parser::ast;

use super::super::super::model::CallInfo;
use super::super::super::source::dotted;
use super::super::shapes::{TypeShape, semantic_decorator};
use super::Resolver;
use super::callback_values::function_returns_parameter;

impl Resolver<'_> {
    pub(super) fn index_decorators(&mut self) {
        let mut functions = self
            .facts
            .functions
            .values()
            .map(|info| (info.qname.clone(), info.node_id.clone()))
            .collect::<Vec<_>>();
        functions.sort_by(|left, right| left.0.cmp(&right.0));

        for (_, node_id) in functions {
            let Some(info) = self.facts.functions.get(&node_id).cloned() else {
                continue;
            };
            if info.is_lambda || info.decorators.is_empty() {
                continue;
            }
            let mut current = BTreeSet::from([info.node_id.clone()]);
            for decorator in info.decorators.iter().rev() {
                let Some(reference) = dotted(decorator) else {
                    continue;
                };
                let leaf = reference.rsplit('.').next().unwrap_or(&reference);
                if semantic_decorator(leaf) || matches!(decorator, ast::Expr::Call(_)) {
                    continue;
                }
                let parent = self.facts.scope_parents.get(&info.node_id).cloned();
                let (decorator_id, _) = self.reference(
                    &info.module_name,
                    info.class_qname.as_deref(),
                    Some(&reference),
                    parent.as_deref(),
                );
                let Some(decorator_info) = decorator_id
                    .as_deref()
                    .and_then(|id| self.facts.functions.get(id))
                    .cloned()
                else {
                    continue;
                };
                let Some(parameter) = decorator_info.parameters.first().map(|item| item.0.clone())
                else {
                    continue;
                };
                let key = (decorator_info.node_id.clone(), parameter.clone());
                let incoming = TypeShape {
                    callables: current.clone(),
                    ..TypeShape::default()
                };
                let previous = self
                    .decorator_argument_shapes
                    .remove(&key)
                    .unwrap_or_default();
                self.decorator_argument_shapes
                    .insert(key.clone(), previous.merge(incoming));
                self.parameter_cache.remove(&key);
                self.return_cache.remove(&decorator_info.node_id);

                if function_returns_parameter(&decorator_info, &parameter) {
                    continue;
                }
                let returned =
                    self.function_return_shape(&decorator_info.node_id, &mut BTreeSet::new());
                if !returned.callables.is_empty() {
                    current = returned.callables;
                }
            }
            if current != BTreeSet::from([info.node_id.clone()]) {
                self.effective_targets.insert(info.node_id, current);
            }
        }
    }

    pub(super) fn index_direct_callers(&mut self) -> BTreeMap<String, Vec<usize>> {
        let mut callers: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for index in 0..self.calls.len() {
            let call = self.calls[index].clone();
            for target in self.direct_syntactic_targets(&call) {
                callers.entry(target).or_default().push(index);
            }
        }
        callers
    }

    fn direct_syntactic_targets(&mut self, call: &CallInfo) -> BTreeSet<String> {
        match &call.callee {
            ast::Expr::Name(value) => {
                if self.name_is_locally_bound(call, value.id.as_str()) {
                    return BTreeSet::new();
                }
                let (target, _) = self.reference(
                    &call.module_name,
                    call.class_qname.as_deref(),
                    Some(value.id.as_str()),
                    Some(&call.scope_id),
                );
                target
                    .filter(|id| matches!(self.kind(id), Some("function" | "method" | "type")))
                    .map(|id| self.effective_target_ids(&id))
                    .unwrap_or_default()
            }
            ast::Expr::Attribute(value) => {
                if let ast::Expr::Call(inner) = value.value.as_ref()
                    && dotted(&inner.func).as_deref() == Some("super")
                {
                    return self
                        .super_method_targets(call.class_qname.as_deref(), value.attr.as_str());
                }
                let reference = dotted(&call.callee);
                let (target, _) = self.reference(
                    &call.module_name,
                    call.class_qname.as_deref(),
                    reference.as_deref(),
                    Some(&call.scope_id),
                );
                target
                    .filter(|id| matches!(self.kind(id), Some("function" | "method" | "type")))
                    .map(|id| self.effective_target_ids(&id))
                    .unwrap_or_default()
            }
            _ => BTreeSet::new(),
        }
    }

    fn name_is_locally_bound(&self, call: &CallInfo, name: &str) -> bool {
        if self
            .facts
            .functions
            .get(&call.scope_id)
            .is_some_and(|info| {
                info.parameters
                    .iter()
                    .any(|(parameter, _)| parameter == name)
            })
        {
            return true;
        }
        let before = u32::from(call.expression_node.range.start());
        self.indexes
            .assignments
            .get(&(call.scope_id.clone(), name.to_owned()))
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|index| self.facts.local_assignments[*index].end <= before)
            })
    }
}

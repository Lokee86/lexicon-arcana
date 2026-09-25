use std::collections::{BTreeMap, BTreeSet};

use rustpython_parser::ast;

use super::super::super::model::{CallInfo, FunctionInfo};
use super::super::super::source::{dotted, offset};
use super::super::shapes::{TypeShape, semantic_decorator};
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn parameter_flow_shape(
        &mut self,
        function_id: &str,
        parameter: &str,
        seen: &mut BTreeSet<(String, String)>,
    ) -> TypeShape {
        let key = (function_id.to_owned(), parameter.to_owned());
        if let Some(value) = self.parameter_cache.get(&key) {
            return value.clone();
        }
        if !self.parameter_active.insert(key.clone()) {
            return TypeShape::default();
        }
        let Some(info) = self.facts.functions.get(function_id).cloned() else {
            self.parameter_active.remove(&key);
            return TypeShape::default();
        };

        let mut shape = self
            .parameter_default_shape(&info, parameter, seen)
            .merge(self.pytest_parametrize_shape(&info, parameter, seen))
            .merge(
                self.decorator_argument_shapes
                    .get(&key)
                    .cloned()
                    .unwrap_or_default(),
            );

        let callers = self
            .direct_callers
            .get(function_id)
            .cloned()
            .unwrap_or_default();
        for call in callers {
            let Some(argument) = self.argument_for_parameter(&call, &info, parameter) else {
                continue;
            };
            shape = shape.merge(self.expression_shape(
                &argument,
                &call.module_name,
                call.class_qname.as_deref(),
                Some(&call.scope_id),
                u32::from(call.expression_node.range.start()),
                seen,
            ));
        }

        self.parameter_active.remove(&key);
        self.parameter_cache.insert(key, shape.clone());
        shape
    }

    fn parameter_default_shape(
        &mut self,
        info: &FunctionInfo,
        parameter: &str,
        seen: &mut BTreeSet<(String, String)>,
    ) -> TypeShape {
        let default = info
            .arguments
            .posonlyargs
            .iter()
            .chain(info.arguments.args.iter())
            .chain(info.arguments.kwonlyargs.iter())
            .find(|item| item.def.arg.as_str() == parameter)
            .and_then(|item| item.default.as_deref())
            .cloned();
        let Some(default) = default else {
            return TypeShape::default();
        };
        let module_scope = self.facts.modules.get(&info.module_name).cloned();
        self.expression_shape(
            &default,
            &info.module_name,
            info.class_qname.as_deref(),
            module_scope.as_deref(),
            offset(&default),
            seen,
        )
    }

    fn pytest_parametrize_shape(
        &mut self,
        info: &FunctionInfo,
        parameter: &str,
        seen: &mut BTreeSet<(String, String)>,
    ) -> TypeShape {
        if info.is_lambda {
            return TypeShape::default();
        }
        let mut shape = TypeShape::default();
        let module_scope = self.facts.modules.get(&info.module_name).cloned();

        for decorator in &info.decorators {
            let ast::Expr::Call(call) = decorator else {
                continue;
            };
            let reference = dotted(&call.func).unwrap_or_default();
            if !reference.ends_with(".parametrize") || call.args.len() < 2 {
                continue;
            }

            let names = parameter_names(&call.args[0]);
            let Some(index) = names.iter().position(|name| name == parameter) else {
                continue;
            };
            for value in parameter_values(&call.args[1], names.len(), index) {
                shape = shape.merge(self.expression_shape(
                    &value,
                    &info.module_name,
                    info.class_qname.as_deref(),
                    module_scope.as_deref(),
                    offset(call),
                    seen,
                ));
            }
        }
        shape
    }

    pub(super) fn index_decorators(&mut self) {
        let mut functions = self.facts.functions.values().cloned().collect::<Vec<_>>();
        functions.sort_by(|left, right| left.qname.cmp(&right.qname));

        for info in functions {
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

    pub(super) fn index_direct_callers(&mut self) -> BTreeMap<String, Vec<CallInfo>> {
        let mut callers: BTreeMap<String, Vec<CallInfo>> = BTreeMap::new();
        let calls = self.facts.calls.clone();
        for call in calls {
            for target in self.direct_syntactic_targets(&call) {
                callers.entry(target).or_default().push(call.clone());
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
            .is_some_and(|items| items.iter().any(|item| item.end <= before))
    }

    fn argument_for_parameter(
        &self,
        call: &CallInfo,
        info: &FunctionInfo,
        parameter: &str,
    ) -> Option<ast::Expr> {
        if let Some(value) = call.expression_node.keywords.iter().find_map(|keyword| {
            keyword
                .arg
                .as_ref()
                .filter(|arg| arg.as_str() == parameter)
                .map(|_| keyword.value.clone())
        }) {
            return Some(value);
        }

        let names = info
            .parameters
            .iter()
            .map(|item| item.0.as_str())
            .collect::<Vec<_>>();
        let mut index = names.iter().position(|name| *name == parameter)?;
        if self.kind(&info.node_id) == Some("method")
            && matches!(call.callee, ast::Expr::Attribute(_))
            && names
                .first()
                .is_some_and(|name| matches!(*name, "self" | "cls"))
        {
            if index == 0 {
                return None;
            }
            index -= 1;
        }
        call.expression_node.args.get(index).cloned()
    }
}

fn function_returns_parameter(info: &FunctionInfo, parameter: &str) -> bool {
    !info.return_expressions.is_empty()
        && info.return_expressions.iter().all(|expression| {
            matches!(
                expression,
                ast::Expr::Name(value) if value.id.as_str() == parameter
            )
        })
}

fn parameter_names(expression: &ast::Expr) -> Vec<String> {
    match expression {
        ast::Expr::Constant(value) => match &value.value {
            ast::Constant::Str(value) => value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        },
        ast::Expr::List(value) => constant_strings(&value.elts),
        ast::Expr::Tuple(value) => constant_strings(&value.elts),
        _ => Vec::new(),
    }
}

fn constant_strings(values: &[ast::Expr]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| match value {
            ast::Expr::Constant(value) => match &value.value {
                ast::Constant::Str(value) => Some(value.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn parameter_values(expression: &ast::Expr, width: usize, index: usize) -> Vec<ast::Expr> {
    let rows = match expression {
        ast::Expr::List(value) => &value.elts,
        ast::Expr::Tuple(value) => &value.elts,
        ast::Expr::Set(value) => &value.elts,
        _ => return Vec::new(),
    };
    rows.iter()
        .filter_map(|row| {
            if width <= 1 {
                return Some(row.clone());
            }
            match row {
                ast::Expr::List(value) => value.elts.get(index).cloned(),
                ast::Expr::Tuple(value) => value.elts.get(index).cloned(),
                _ => None,
            }
        })
        .collect()
}

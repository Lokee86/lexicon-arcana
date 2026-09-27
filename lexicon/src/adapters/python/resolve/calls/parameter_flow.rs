use std::collections::BTreeSet;

use rustpython_parser::ast;

use super::super::super::model::FunctionInfo;
use super::super::super::source::{dotted, offset};
use super::super::shapes::TypeShape;
use super::Resolver;
use super::callback_values::{parameter_names, parameter_values};

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
        for index in callers {
            let call = self.calls[index].clone();
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

    fn argument_for_parameter(
        &self,
        call: &super::super::super::model::CallInfo,
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

use std::collections::BTreeSet;

use rustpython_parser::ast;

use super::super::super::source::{dotted, offset};
use super::super::shapes::TypeShape;
use super::Resolver;

impl Resolver<'_> {
    pub(super) fn expression_shape(
        &mut self,
        expression: &ast::Expr,
        module: &str,
        class_qname: Option<&str>,
        scope: Option<&str>,
        before: u32,
        seen: &mut BTreeSet<(String, String)>,
    ) -> TypeShape {
        match expression {
            ast::Expr::Name(value) => {
                if let Some(class_qname) = class_qname
                    && matches!(value.id.as_str(), "self" | "cls")
                    && let Some(class_id) = self.facts.symbols.get(class_qname).cloned()
                {
                    let direct = self.instance_type_ids(&class_id);
                    return TypeShape {
                        callables: if value.id.as_str() == "cls" {
                            direct.clone()
                        } else {
                            BTreeSet::new()
                        },
                        direct,
                        ..TypeShape::default()
                    };
                }

                let local =
                    self.local_shape(value.id.as_str(), module, class_qname, scope, before, seen);
                if local != TypeShape::default() {
                    return local;
                }

                let imported = self.imported_value_shape(value.id.as_str(), module, scope, seen);
                if imported != TypeShape::default() {
                    return imported;
                }

                let (target, reason) =
                    self.reference(module, class_qname, Some(value.id.as_str()), scope);
                if target.is_some() {
                    self.shape_for_reference(target)
                } else if matches!(
                    reason.as_str(),
                    "builtin-target" | "external-target" | "dynamic-target"
                ) {
                    TypeShape::runtime(&reason)
                } else {
                    TypeShape::default()
                }
            }
            ast::Expr::Attribute(value) => {
                if value.attr.as_str() == "__dict__" {
                    return TypeShape::runtime("builtin-target");
                }
                let reference = dotted(expression);
                let (target, _) = self.reference(module, class_qname, reference.as_deref(), scope);
                let direct = self.shape_for_reference(target);
                if direct != TypeShape::default() {
                    return direct;
                }

                let receiver =
                    self.expression_shape(&value.value, module, class_qname, scope, before, seen);
                let mut result = TypeShape::default();
                let classes = receiver.direct.iter().cloned().collect::<Vec<_>>();
                for class_id in classes {
                    if let Some(qname) = self.facts.qnames.get(&class_id).cloned() {
                        result = result.merge(self.field_shape(&qname, value.attr.as_str(), seen));
                    }
                    result
                        .callables
                        .extend(self.method_targets(&class_id, value.attr.as_str()));
                }
                if result == TypeShape::default() && !receiver.runtime_reasons.is_empty() {
                    TypeShape {
                        runtime_reasons: receiver.runtime_reasons,
                        ..TypeShape::default()
                    }
                } else {
                    result
                }
            }
            ast::Expr::Lambda(value) => self
                .facts
                .lambda_ids
                .get(&(module.to_owned(), offset(value)))
                .cloned()
                .map(TypeShape::callable)
                .unwrap_or_default(),
            ast::Expr::Subscript(value) => {
                let container =
                    self.expression_shape(&value.value, module, class_qname, scope, before, seen);
                let element = container.element_shape();
                if element != TypeShape::default() {
                    element
                } else {
                    TypeShape {
                        runtime_reasons: container.runtime_reasons,
                        ..TypeShape::default()
                    }
                }
            }
            ast::Expr::Call(value) => {
                let reference = dotted(&value.func);
                if matches!(reference.as_deref(), Some("functools.partial" | "partial"))
                    && let Some(target) = value.args.first()
                {
                    let shape = self.expression_shape(
                        target,
                        module,
                        class_qname,
                        scope,
                        offset(value),
                        seen,
                    );
                    if !shape.callables.is_empty() {
                        return TypeShape {
                            callables: shape.callables,
                            ..TypeShape::default()
                        };
                    }
                }

                if reference.as_deref() == Some("getattr")
                    && value.args.len() >= 2
                    && let ast::Expr::Constant(attribute) = &value.args[1]
                    && let ast::Constant::Str(name) = &attribute.value
                {
                    let receiver = self.expression_shape(
                        &value.args[0],
                        module,
                        class_qname,
                        scope,
                        offset(value),
                        seen,
                    );
                    let mut result = TypeShape::default();
                    let classes = receiver.direct.iter().cloned().collect::<Vec<_>>();
                    for class_id in classes {
                        if let Some(qname) = self.facts.qnames.get(&class_id).cloned() {
                            result = result.merge(self.field_shape(&qname, name, seen));
                        }
                        result
                            .callables
                            .extend(self.method_targets(&class_id, name));
                    }
                    if result != TypeShape::default() {
                        return result;
                    }
                }

                if let ast::Expr::Attribute(attribute) = value.func.as_ref()
                    && matches!(attribute.attr.as_str(), "get" | "pop" | "setdefault")
                {
                    let container = self.expression_shape(
                        &attribute.value,
                        module,
                        class_qname,
                        scope,
                        offset(value),
                        seen,
                    );
                    let mut element = container.element_shape();
                    if attribute.attr.as_str() == "setdefault"
                        && let Some(default) = value.args.get(1)
                    {
                        element = element.merge(self.expression_shape(
                            default,
                            module,
                            class_qname,
                            scope,
                            offset(value),
                            seen,
                        ));
                    }
                    if element != TypeShape::default() {
                        return element;
                    }
                }

                let (targets, reason) = self.callable_targets(
                    &value.func,
                    module,
                    class_qname,
                    scope,
                    offset(value),
                    seen,
                );
                let mut result = TypeShape::default();
                for target in targets {
                    match self.kind(&target) {
                        Some("type") => {
                            result.direct.insert(target);
                        }
                        Some("function" | "method") => {
                            result = result.merge(self.function_return_shape(&target, seen));
                        }
                        _ => {}
                    }
                }
                if result == TypeShape::default()
                    && matches!(
                        reason.as_str(),
                        "builtin-target" | "external-target" | "dynamic-target"
                    )
                {
                    TypeShape::runtime(&reason)
                } else {
                    result
                }
            }
            ast::Expr::Constant(_) | ast::Expr::JoinedStr(_) => {
                TypeShape::runtime("builtin-target")
            }
            ast::Expr::BinOp(value) => {
                let left =
                    self.expression_shape(&value.left, module, class_qname, scope, before, seen);
                if matches!(value.op, ast::Operator::Div) && left != TypeShape::default() {
                    left
                } else {
                    left.merge(self.expression_shape(
                        &value.right,
                        module,
                        class_qname,
                        scope,
                        before,
                        seen,
                    ))
                }
            }
            ast::Expr::UnaryOp(value) => {
                self.expression_shape(&value.operand, module, class_qname, scope, before, seen)
            }
            ast::Expr::Compare(_) | ast::Expr::FormattedValue(_) => {
                TypeShape::runtime("builtin-target")
            }
            ast::Expr::IfExp(value) => self
                .expression_shape(&value.body, module, class_qname, scope, before, seen)
                .merge(self.expression_shape(
                    &value.orelse,
                    module,
                    class_qname,
                    scope,
                    before,
                    seen,
                )),
            ast::Expr::BoolOp(value) => {
                let mut shape = TypeShape::default();
                for item in &value.values {
                    shape = shape.merge(self.expression_shape(
                        item,
                        module,
                        class_qname,
                        scope,
                        before,
                        seen,
                    ));
                }
                shape
            }
            ast::Expr::NamedExpr(value) => {
                self.expression_shape(&value.value, module, class_qname, scope, before, seen)
            }
            ast::Expr::Await(value) => {
                self.expression_shape(&value.value, module, class_qname, scope, before, seen)
            }
            ast::Expr::List(value) => {
                self.element_shapes(&value.elts, module, class_qname, scope, before, seen)
            }
            ast::Expr::Set(value) => {
                self.element_shapes(&value.elts, module, class_qname, scope, before, seen)
            }
            ast::Expr::Tuple(value) => {
                self.element_shapes(&value.elts, module, class_qname, scope, before, seen)
            }
            ast::Expr::Dict(value) => {
                self.element_shapes(&value.values, module, class_qname, scope, before, seen)
            }
            ast::Expr::ListComp(value) => TypeShape::elements([self.expression_shape(
                &value.elt,
                module,
                class_qname,
                scope,
                before,
                seen,
            )]),
            ast::Expr::SetComp(value) => TypeShape::elements([self.expression_shape(
                &value.elt,
                module,
                class_qname,
                scope,
                before,
                seen,
            )]),
            ast::Expr::GeneratorExp(value) => TypeShape::elements([self.expression_shape(
                &value.elt,
                module,
                class_qname,
                scope,
                before,
                seen,
            )]),
            ast::Expr::DictComp(value) => TypeShape::elements([self.expression_shape(
                &value.value,
                module,
                class_qname,
                scope,
                before,
                seen,
            )]),
            _ => TypeShape::default(),
        }
    }

    fn element_shapes(
        &mut self,
        values: &[ast::Expr],
        module: &str,
        class_qname: Option<&str>,
        scope: Option<&str>,
        before: u32,
        seen: &mut BTreeSet<(String, String)>,
    ) -> TypeShape {
        let mut shapes = Vec::with_capacity(values.len());
        for item in values {
            shapes.push(self.expression_shape(item, module, class_qname, scope, before, seen));
        }
        TypeShape::elements(shapes)
    }
}

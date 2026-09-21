"""Python call-graph semantic layer."""

from __future__ import annotations

import ast

from .bindings import dotted
from .callgraph_shapes import (
    _CONTAINER_ACCESSORS,
    _EMPTY,
    TypeShape,
    _elements_from_shapes,
    _merge_shapes,
)


class ExpressionFlow:
    def expression_shape(
        self,
        expression: ast.expr | None,
        module_name: str,
        class_qname: str | None,
        scope_id: str | None,
        before: ast.AST,
        seen: set[tuple[str, str]],
    ) -> TypeShape:
        if expression is None:
            return _EMPTY
        if isinstance(expression, ast.Name):
            if class_qname and expression.id in {"self", "cls"}:
                class_id = self.facts.symbols.get(class_qname)
                if class_id is None:
                    return _EMPTY
                instance_types = self._instance_type_ids(class_id)
                return TypeShape(
                    direct=instance_types,
                    callables=instance_types if expression.id == "cls" else frozenset(),
                )
            local = self._local_shape(
                expression.id,
                module_name,
                class_qname,
                scope_id,
                before,
                seen,
            )
            if local != _EMPTY:
                return local
            imported = self._imported_value_shape(
                expression.id,
                module_name,
                scope_id,
                seen,
            )
            if imported != _EMPTY:
                return imported
            target_id, reason = self.bindings.resolve_reference(
                module_name,
                class_qname,
                expression.id,
                scope_id,
            )
            if target_id:
                return self._shape_for_reference(target_id)
            if reason in {"builtin-target", "external-target", "dynamic-target"}:
                return TypeShape(runtime_reasons=frozenset({reason}))
            return _EMPTY
        if isinstance(expression, ast.Attribute):
            if expression.attr == "__dict__":
                return TypeShape(runtime_reasons=frozenset({"builtin-target"}))
            direct_id, _ = self.bindings.resolve_reference(
                module_name,
                class_qname,
                dotted(expression),
                scope_id,
            )
            direct_shape = self._shape_for_reference(direct_id)
            if direct_shape != _EMPTY:
                return direct_shape
            receiver = self.expression_shape(
                expression.value,
                module_name,
                class_qname,
                scope_id,
                before,
                seen,
            )
            result = _EMPTY
            method_targets: set[str] = set()
            for class_id in receiver.direct:
                class_name = self.facts.node_qnames.get(class_id)
                if class_name:
                    result = result.merge(self._field_shape(class_name, expression.attr, seen))
                method_targets.update(self._method_targets(class_id, expression.attr))
            if method_targets:
                result = result.merge(TypeShape(callables=frozenset(method_targets)))
            if result == _EMPTY and receiver.runtime_reasons:
                return TypeShape(runtime_reasons=receiver.runtime_reasons)
            return result
        if isinstance(expression, ast.Lambda):
            lambda_id = self.facts.lambda_ids.get((module_name, expression.lineno, expression.col_offset))
            return TypeShape(callables=frozenset({lambda_id})) if lambda_id else _EMPTY
        if isinstance(expression, ast.Subscript):
            container = self.expression_shape(
                expression.value,
                module_name,
                class_qname,
                scope_id,
                before,
                seen,
            )
            element = container.element_shape()
            if element != _EMPTY:
                return element
            return TypeShape(runtime_reasons=container.runtime_reasons)
        if isinstance(expression, ast.Call):
            reference = dotted(expression.func)
            if reference in {"functools.partial", "partial"} and expression.args:
                target_shape = self.expression_shape(
                    expression.args[0],
                    module_name,
                    class_qname,
                    scope_id,
                    expression,
                    seen,
                )
                if target_shape.callables:
                    return TypeShape(callables=target_shape.callables)
            if reference == "getattr" and len(expression.args) >= 2:
                attribute_name = expression.args[1]
                if isinstance(attribute_name, ast.Constant) and isinstance(attribute_name.value, str):
                    receiver = self.expression_shape(
                        expression.args[0],
                        module_name,
                        class_qname,
                        scope_id,
                        expression,
                        seen,
                    )
                    result = _EMPTY
                    method_targets: set[str] = set()
                    for class_id in receiver.direct:
                        class_name = self.facts.node_qnames.get(class_id)
                        if class_name:
                            result = result.merge(
                                self._field_shape(class_name, attribute_name.value, seen)
                            )
                        method_targets.update(
                            self._method_targets(class_id, attribute_name.value)
                        )
                    if method_targets:
                        result = result.merge(
                            TypeShape(callables=frozenset(method_targets))
                        )
                    if result != _EMPTY:
                        return result
            if isinstance(expression.func, ast.Attribute) and expression.func.attr in _CONTAINER_ACCESSORS:
                container = self.expression_shape(
                    expression.func.value,
                    module_name,
                    class_qname,
                    scope_id,
                    expression,
                    seen,
                )
                element = container.element_shape()
                if element != _EMPTY:
                    if expression.func.attr == "setdefault" and len(expression.args) >= 2:
                        element = element.merge(
                            self.expression_shape(
                                expression.args[1],
                                module_name,
                                class_qname,
                                scope_id,
                                expression,
                                seen,
                            )
                        )
                    return element
            targets, reason = self._callable_targets(
                expression.func,
                module_name,
                class_qname,
                scope_id,
                expression,
                seen,
            )
            result = _EMPTY
            for target_id in targets:
                kind = self._kind(target_id)
                if kind == "type":
                    result = result.merge(TypeShape(direct=frozenset({target_id})))
                elif kind in {"function", "method"}:
                    result = result.merge(self.function_return_shape(target_id))
            if result == _EMPTY and reason in {
                "builtin-target",
                "external-target",
                "dynamic-target",
            }:
                return TypeShape(runtime_reasons=frozenset({reason}))
            return result
        if isinstance(expression, ast.Constant):
            return TypeShape(runtime_reasons=frozenset({"builtin-target"}))
        if isinstance(expression, ast.JoinedStr):
            return TypeShape(runtime_reasons=frozenset({"builtin-target"}))
        if isinstance(expression, ast.BinOp):
            left = self.expression_shape(
                expression.left, module_name, class_qname, scope_id, before, seen
            )
            if isinstance(expression.op, ast.Div) and left != _EMPTY:
                return left
            return left.merge(
                self.expression_shape(
                    expression.right, module_name, class_qname, scope_id, before, seen
                )
            )
        if isinstance(expression, ast.UnaryOp):
            return self.expression_shape(
                expression.operand, module_name, class_qname, scope_id, before, seen
            )
        if isinstance(expression, (ast.Compare, ast.FormattedValue)):
            return TypeShape(runtime_reasons=frozenset({"builtin-target"}))
        if isinstance(expression, ast.IfExp):
            return self.expression_shape(expression.body, module_name, class_qname, scope_id, before, seen).merge(
                self.expression_shape(expression.orelse, module_name, class_qname, scope_id, before, seen)
            )
        if isinstance(expression, ast.BoolOp):
            return _merge_shapes(
                [
                    self.expression_shape(item, module_name, class_qname, scope_id, before, seen)
                    for item in expression.values
                ]
            )
        if isinstance(expression, ast.NamedExpr):
            return self.expression_shape(expression.value, module_name, class_qname, scope_id, before, seen)
        if isinstance(expression, ast.Await):
            return self.expression_shape(expression.value, module_name, class_qname, scope_id, before, seen)
        if isinstance(expression, (ast.List, ast.Set, ast.Tuple)):
            return _elements_from_shapes(
                [
                    self.expression_shape(item, module_name, class_qname, scope_id, before, seen)
                    for item in expression.elts
                ]
            )
        if isinstance(expression, ast.Dict):
            return _elements_from_shapes(
                [
                    self.expression_shape(item, module_name, class_qname, scope_id, before, seen)
                    for item in expression.values
                    if item is not None
                ]
            )
        if isinstance(expression, (ast.ListComp, ast.SetComp, ast.GeneratorExp)):
            return _elements_from_shapes(
                [self.expression_shape(expression.elt, module_name, class_qname, scope_id, expression, seen)]
            )
        if isinstance(expression, ast.DictComp):
            return _elements_from_shapes(
                [self.expression_shape(expression.value, module_name, class_qname, scope_id, expression, seen)]
            )
        return _EMPTY

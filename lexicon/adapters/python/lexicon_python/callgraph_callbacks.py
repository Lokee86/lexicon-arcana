"""Python call-graph semantic layer."""

from __future__ import annotations

import ast

from .bindings import dotted
from .callgraph_shapes import (
    _CALLABLE_KINDS,
    _EMPTY,
    _SEMANTIC_DECORATORS,
    TypeShape,
    _position,
    _precedes,
    _return_expressions,
)
from .model import CallInfo, FunctionInfo


class CallbackFlow:
    def _parameter_flow_shape(
        self,
        function_id: str,
        parameter_name: str,
        seen: set[tuple[str, str]],
    ) -> TypeShape:
        key = (function_id, parameter_name)
        if key in self._parameter_cache:
            return self._parameter_cache[key]
        if key in self._parameter_active:
            return _EMPTY
        info = self.facts.functions.get(function_id)
        if info is None:
            return _EMPTY
        self._parameter_active.add(key)
        shape = self._parameter_default_shape(info, parameter_name, seen).merge(
            self._pytest_parametrize_shape(info, parameter_name, seen)
        ).merge(self._decorator_argument_shapes.get(key, _EMPTY))
        for call in self._direct_callers.get(function_id, ()):
            argument = self._argument_for_parameter(call, info, parameter_name)
            if argument is None:
                continue
            shape = shape.merge(
                self.expression_shape(
                    argument,
                    call.module_name,
                    call.class_qname,
                    call.scope_id,
                    call.expression_node,
                    {*seen, ("parameter-argument", f"{function_id}:{parameter_name}")},
                )
            )
        self._parameter_active.remove(key)
        self._parameter_cache[key] = shape
        return shape

    def _parameter_default_shape(
        self,
        info: FunctionInfo,
        parameter_name: str,
        seen: set[tuple[str, str]],
    ) -> TypeShape:
        node = info.node
        if isinstance(node, ast.Lambda):
            arguments = node.args
        else:
            arguments = node.args
        positional = [*arguments.posonlyargs, *arguments.args]
        defaults_by_name: dict[str, ast.expr] = {}
        if arguments.defaults:
            for argument, default in zip(positional[-len(arguments.defaults):], arguments.defaults):
                defaults_by_name[argument.arg] = default
        for argument, default in zip(arguments.kwonlyargs, arguments.kw_defaults):
            if default is not None:
                defaults_by_name[argument.arg] = default
        default = defaults_by_name.get(parameter_name)
        if default is None:
            return _EMPTY
        module_scope = self.facts.modules.get(info.module_name)
        return self.expression_shape(
            default,
            info.module_name,
            info.class_qname,
            module_scope,
            default,
            seen,
        )

    def _pytest_parametrize_shape(
        self,
        info: FunctionInfo,
        parameter_name: str,
        seen: set[tuple[str, str]],
    ) -> TypeShape:
        if isinstance(info.node, ast.Lambda):
            return _EMPTY
        shape = _EMPTY
        module_scope = self.facts.modules.get(info.module_name)
        for decorator in info.node.decorator_list:
            if not isinstance(decorator, ast.Call):
                continue
            reference = dotted(decorator.func) or ""
            if not reference.endswith(".parametrize") or len(decorator.args) < 2:
                continue
            names_node = decorator.args[0]
            if isinstance(names_node, ast.Constant) and isinstance(names_node.value, str):
                names = [name.strip() for name in names_node.value.split(",")]
            elif isinstance(names_node, (ast.List, ast.Tuple)):
                names = [
                    item.value
                    for item in names_node.elts
                    if isinstance(item, ast.Constant) and isinstance(item.value, str)
                ]
            else:
                continue
            if parameter_name not in names:
                continue
            parameter_index = names.index(parameter_name)
            values_node = decorator.args[1]
            if not isinstance(values_node, (ast.List, ast.Tuple, ast.Set)):
                continue
            for row in values_node.elts:
                value = row
                if len(names) > 1:
                    if not isinstance(row, (ast.List, ast.Tuple)) or parameter_index >= len(row.elts):
                        continue
                    value = row.elts[parameter_index]
                shape = shape.merge(
                    self.expression_shape(
                        value,
                        info.module_name,
                        info.class_qname,
                        module_scope,
                        decorator,
                        seen,
                    )
                )
        return shape

    def _index_decorators(self) -> None:
        for info in sorted(self.facts.functions.values(), key=lambda item: item.qname):
            if isinstance(info.node, ast.Lambda) or not info.node.decorator_list:
                continue
            current = frozenset({info.node_id})
            for decorator in reversed(info.node.decorator_list):
                reference = dotted(decorator)
                if reference is None or reference.rsplit(".", 1)[-1] in _SEMANTIC_DECORATORS:
                    continue
                if isinstance(decorator, ast.Call):
                    continue
                parent_scope = self.facts.scope_parents.get(info.node_id)
                decorator_id, _ = self.bindings.resolve_reference(
                    info.module_name,
                    info.class_qname,
                    reference,
                    parent_scope,
                )
                decorator_info = self.facts.functions.get(decorator_id or "")
                if decorator_info is None:
                    continue
                parameter_name = next(iter(decorator_info.parameters), None)
                if parameter_name is None:
                    continue
                key = (decorator_info.node_id, parameter_name)
                self._decorator_argument_shapes[key] = self._decorator_argument_shapes.get(
                    key, _EMPTY
                ).merge(TypeShape(callables=current))
                self._parameter_cache.pop(key, None)
                self._return_cache.pop(decorator_info.node_id, None)
                if self._function_returns_parameter(decorator_info, parameter_name):
                    continue
                returned = self.function_return_shape(decorator_info.node_id)
                if returned.callables:
                    current = returned.callables
            if current != frozenset({info.node_id}):
                self._effective_targets[info.node_id] = current

    def _function_returns_parameter(
        self,
        info: FunctionInfo,
        parameter_name: str,
    ) -> bool:
        expressions = _return_expressions(info.node)
        return bool(expressions) and all(
            isinstance(expression, ast.Name) and expression.id == parameter_name
            for expression in expressions
        )

    def _effective_target_ids(self, identifier: str) -> frozenset[str]:
        return self._effective_targets.get(identifier, frozenset({identifier}))

    def _index_direct_callers(self) -> dict[str, tuple[CallInfo, ...]]:
        callers: dict[str, list[CallInfo]] = {}
        for call in self.facts.calls:
            target_ids = self._direct_syntactic_targets(call)
            for target_id in target_ids:
                callers.setdefault(target_id, []).append(call)
        return {target_id: tuple(items) for target_id, items in callers.items()}

    def _direct_syntactic_targets(self, call: CallInfo) -> set[str]:
        callee = call.callee
        if isinstance(callee, ast.Name):
            if self._name_is_locally_bound(call, callee.id):
                return set()
            target_id, _ = self.bindings.resolve_reference(
                call.module_name,
                call.class_qname,
                callee.id,
                call.scope_id,
            )
            return set(self._effective_target_ids(target_id)) if target_id and self._kind(target_id) in _CALLABLE_KINDS else set()
        if isinstance(callee, ast.Attribute):
            if (
                isinstance(callee.value, ast.Call)
                and isinstance(callee.value.func, ast.Name)
                and callee.value.func.id == "super"
            ):
                return self._super_method_targets(call.class_qname, callee.attr)
            target_id, _ = self.bindings.resolve_reference(
                call.module_name,
                call.class_qname,
                dotted(callee),
                call.scope_id,
            )
            return set(self._effective_target_ids(target_id)) if target_id and self._kind(target_id) in _CALLABLE_KINDS else set()
        return set()

    def _name_is_locally_bound(self, call: CallInfo, name: str) -> bool:
        info = self.facts.functions.get(call.scope_id or "")
        if info and name in info.parameters:
            return True
        if call.scope_id is None:
            return False
        return any(
            _precedes(assignment.assignment_node, call.expression_node)
            for assignment in self._indexes.assignments_by_scope_name.get(
                (call.scope_id, name),
                (),
            )
        )

    def _argument_for_parameter(
        self,
        call: CallInfo,
        info: FunctionInfo,
        parameter_name: str,
    ) -> ast.expr | None:
        keyword = next((item.value for item in call.expression_node.keywords if item.arg == parameter_name), None)
        if keyword is not None:
            return keyword
        parameter_names = list(info.parameters)
        bound_offset = 0
        if self._kind(info.node_id) == "method" and isinstance(call.callee, ast.Attribute):
            if parameter_names and parameter_names[0] in {"self", "cls"}:
                bound_offset = 1
        try:
            parameter_index = parameter_names.index(parameter_name) - bound_offset
        except ValueError:
            return None
        if parameter_index < 0 or parameter_index >= len(call.expression_node.args):
            return None
        return call.expression_node.args[parameter_index]

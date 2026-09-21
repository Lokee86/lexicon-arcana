"""Lexical and assignment value-flow resolution."""

from __future__ import annotations

import ast

from .bindings import resolve_relative_module
from .callgraph_shapes import _EMPTY, TypeShape, _position, _precedes


class ScopeFlow:
    def _local_shape(
        self,
        name: str,
        module_name: str,
        class_qname: str | None,
        scope_id: str | None,
        before: ast.AST | None,
        seen: set[tuple[str, str]],
    ) -> TypeShape:
        if scope_id is None:
            return _EMPTY
        marker = (scope_id, name)
        if marker in seen:
            return _EMPTY
        next_seen = {*seen, marker}
        info = self.facts.functions.get(scope_id)
        shape = _EMPTY
        has_local_evidence = False
        if info and name in info.parameters:
            has_local_evidence = True
            shape = self.annotation_shape(
                info.parameters[name], module_name, class_qname, scope_id
            ).merge(self._parameter_flow_shape(scope_id, name, next_seen))
        assignments = sorted(
            (
                assignment
                for assignment in self._indexes.assignments_by_scope_name.get(
                    (scope_id, name),
                    (),
                )
                if before is None
                or _precedes(assignment.assignment_node, before)
            ),
            key=lambda assignment: _position(assignment.assignment_node),
        )
        for assignment in assignments:
            has_local_evidence = True
            candidate = self.annotation_shape(
                assignment.annotation,
                assignment.module_name,
                assignment.class_qname,
                assignment.scope_id,
            )
            candidate = candidate.merge(
                self.expression_shape(
                    assignment.value,
                    assignment.module_name,
                    assignment.class_qname,
                    assignment.scope_id,
                    assignment.assignment_node,
                    next_seen,
                )
            )
            shape = shape.merge(candidate) if assignment.branch_dependent else candidate
        loops = sorted(
            (
                binding
                for binding in self._indexes.loop_bindings_by_scope_name.get(
                    (scope_id, name),
                    (),
                )
                if before is None
                or _position(binding.loop_node) <= _position(before)
            ),
            key=lambda binding: _position(binding.loop_node),
        )
        for binding in loops:
            has_local_evidence = True
            if (
                binding.element_index == 1
                and isinstance(binding.iterable, ast.Call)
                and isinstance(binding.iterable.func, ast.Attribute)
                and binding.iterable.func.attr == "items"
            ):
                container = self.expression_shape(
                    binding.iterable.func.value,
                    binding.module_name,
                    binding.class_qname,
                    binding.scope_id,
                    binding.loop_node,
                    next_seen,
                )
                candidate = container.element_shape()
                if candidate == _EMPTY:
                    candidate = TypeShape(runtime_reasons=container.runtime_reasons)
            elif binding.element_index is not None:
                candidate = _EMPTY
            else:
                iterable = self.expression_shape(
                    binding.iterable,
                    binding.module_name,
                    binding.class_qname,
                    binding.scope_id,
                    binding.loop_node,
                    next_seen,
                )
                candidate = iterable.element_shape()
                if candidate == _EMPTY:
                    candidate = TypeShape(runtime_reasons=iterable.runtime_reasons)
            shape = shape.merge(candidate) if binding.branch_dependent else candidate
        if not has_local_evidence:
            enclosing_scope = self._enclosing_value_scope(scope_id, module_name)
            if enclosing_scope and enclosing_scope != scope_id:
                enclosing_function = self.facts.functions.get(enclosing_scope)
                return self._local_shape(
                    name,
                    module_name,
                    enclosing_function.class_qname if enclosing_function else None,
                    enclosing_scope,
                    before,
                    next_seen,
                )
        return shape

    def _imported_value_shape(
        self,
        name: str,
        module_name: str,
        scope_id: str | None,
        seen: set[tuple[str, str]],
    ) -> TypeShape:
        module_scope = self.facts.modules.get(module_name)
        owners = tuple(
            owner for owner in (scope_id, module_scope) if owner is not None
        )
        shape = _EMPTY
        for owner in owners:
            for info in self._indexes.imports_by_owner_binding.get(
                (owner, name),
                (),
            ):
                if not info.target_name:
                    continue
                requested_module = resolve_relative_module(info)
                target_module = self.bindings.resolve_module_name(
                    requested_module, info.module_name
                )
                target_scope = self.facts.modules.get(target_module or "")
                if not target_module or not target_scope:
                    continue
                shape = shape.merge(
                    self._local_shape(
                        info.target_name,
                        target_module,
                        None,
                        target_scope,
                        None,
                        {
                            *seen,
                            (
                                "imported-value",
                                f"{target_module}:{info.target_name}",
                            ),
                        },
                    )
                )
        return shape

    def _enclosing_value_scope(self, scope_id: str, module_name: str) -> str | None:
        parent = self.facts.scope_parents.get(scope_id)
        while parent:
            kind = self._kind(parent)
            if kind in {"function", "method", "module"}:
                return parent
            parent = self.facts.scope_parents.get(parent)
        return self.facts.modules.get(module_name)

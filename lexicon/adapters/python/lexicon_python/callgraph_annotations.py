"""Python call-graph semantic layer."""

from __future__ import annotations

import ast

from .bindings import dotted

from .callgraph_shapes import (
    _EMPTY,
    _MAPPING_TYPES,
    _SEQUENCE_TYPES,
    _UNION_TYPES,
    _WRAPPER_TYPES,
    TypeShape,
    _elements_from_shapes,
    _merge_shapes,
    _return_expressions,
)


class AnnotationFlow:
    def annotation_shape(
        self,
        annotation: ast.expr | None,
        module_name: str,
        class_qname: str | None,
        scope_id: str | None,
    ) -> TypeShape:
        if annotation is None:
            return _EMPTY
        if isinstance(annotation, ast.Constant) and isinstance(annotation.value, str):
            try:
                parsed = ast.parse(annotation.value, mode="eval").body
            except SyntaxError:
                return _EMPTY
            return self.annotation_shape(parsed, module_name, class_qname, scope_id)
        if isinstance(annotation, ast.BinOp) and isinstance(annotation.op, ast.BitOr):
            return self.annotation_shape(annotation.left, module_name, class_qname, scope_id).merge(
                self.annotation_shape(annotation.right, module_name, class_qname, scope_id)
            )
        if isinstance(annotation, ast.Subscript):
            origin = (dotted(annotation.value) or "").rsplit(".", 1)[-1]
            arguments = list(annotation.slice.elts) if isinstance(annotation.slice, ast.Tuple) else [annotation.slice]
            shapes = [self.annotation_shape(item, module_name, class_qname, scope_id) for item in arguments]
            if origin in _SEQUENCE_TYPES:
                return _elements_from_shapes(shapes)
            if origin in _MAPPING_TYPES:
                return _elements_from_shapes(shapes[-1:])
            if origin in _UNION_TYPES:
                return _merge_shapes(shapes)
            if origin in _WRAPPER_TYPES:
                return shapes[0] if shapes else _EMPTY
            if origin == "Callable":
                return TypeShape(call_reasons=frozenset({"dynamic-target"}))
            return self.annotation_shape(annotation.value, module_name, class_qname, scope_id)
        reference = dotted(annotation)
        if reference == "Any" or (reference and reference.endswith(".Any")):
            return TypeShape(runtime_reasons=frozenset({"dynamic-target"}))
        target_id, reason = self.bindings.resolve_reference(
            module_name, class_qname, reference, scope_id
        )
        if target_id:
            return self._annotation_shape_for_reference(target_id)
        if reason in {"builtin-target", "external-target", "dynamic-target"}:
            return TypeShape(runtime_reasons=frozenset({reason}))
        return _EMPTY

    def function_return_shape(self, function_id: str) -> TypeShape:
        if function_id in self._return_cache:
            return self._return_cache[function_id]
        if function_id in self._return_active:
            return _EMPTY
        info = self.facts.functions.get(function_id)
        if info is None:
            return _EMPTY
        self._return_active.add(function_id)
        shape = self.annotation_shape(
            info.return_annotation,
            info.module_name,
            info.class_qname,
            info.node_id,
        )
        if shape == _EMPTY:
            for expression in _return_expressions(info.node):
                shape = shape.merge(
                    self.expression_shape(
                        expression,
                        info.module_name,
                        info.class_qname,
                        info.node_id,
                        expression,
                        {("return", function_id)},
                    )
                )
        self._return_active.remove(function_id)
        self._return_cache[function_id] = shape
        return shape

    def _field_shape(self, class_qname: str, field_name: str, seen: set[tuple[str, str]]) -> TypeShape:
        key = (class_qname, field_name)
        if key in self._field_cache:
            return self._field_cache[key]
        marker = (class_qname, f"field:{field_name}")
        if marker in seen:
            return _EMPTY
        next_seen = {*seen, marker}
        info = self.facts.classes.get(class_qname)
        if info is None:
            return _EMPTY
        shape = _EMPTY
        for statement in info.node.body:
            if isinstance(statement, ast.AnnAssign) and isinstance(statement.target, ast.Name) and statement.target.id == field_name:
                shape = shape.merge(
                    self.annotation_shape(statement.annotation, info.module_name, class_qname, None)
                )
            if isinstance(statement, ast.Assign):
                for target in statement.targets:
                    if isinstance(target, ast.Name) and target.id == field_name:
                        shape = shape.merge(
                            self.expression_shape(
                                statement.value,
                                info.module_name,
                                class_qname,
                                info.node_id,
                                statement,
                                next_seen,
                            )
                        )
        for assignment in self._indexes.field_assignments.get(
            (class_qname, field_name),
            (),
        ):
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
            shape = shape.merge(candidate)
        for base_qname in self._base_qnames(class_qname):
            shape = shape.merge(self._field_shape(base_qname, field_name, next_seen))
        self._field_cache[key] = shape
        return shape

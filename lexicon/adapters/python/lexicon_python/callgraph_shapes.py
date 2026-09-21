"""Shared call-graph value shapes and ordering helpers."""

from __future__ import annotations

import ast
from dataclasses import dataclass

_SEQUENCE_TYPES = {
    "AsyncIterable",
    "AsyncIterator",
    "Collection",
    "Generator",
    "Iterable",
    "Iterator",
    "List",
    "Sequence",
    "Set",
    "Tuple",
    "list",
    "set",
    "tuple",
    "frozenset",
}
_MAPPING_TYPES = {"Dict", "Mapping", "MutableMapping", "dict"}
_UNION_TYPES = {"Annotated", "Optional", "Union"}
_WRAPPER_TYPES = {"ClassVar", "Final", "Required", "NotRequired", "Type", "type"}
_CONTAINER_ACCESSORS = {"get", "pop", "setdefault"}
_CALLABLE_KINDS = {"function", "method", "type"}
_SEMANTIC_DECORATORS = {
    "abstractmethod",
    "cached_property",
    "classmethod",
    "dataclass",
    "final",
    "overload",
    "override",
    "property",
    "staticmethod",
}


@dataclass(frozen=True, slots=True)
class TypeShape:
    direct: frozenset[str] = frozenset()
    elements: frozenset[str] = frozenset()
    callables: frozenset[str] = frozenset()
    element_callables: frozenset[str] = frozenset()
    runtime_reasons: frozenset[str] = frozenset()
    call_reasons: frozenset[str] = frozenset()
    element_runtime_reasons: frozenset[str] = frozenset()
    element_call_reasons: frozenset[str] = frozenset()

    def merge(self, other: "TypeShape") -> "TypeShape":
        return TypeShape(
            self.direct | other.direct,
            self.elements | other.elements,
            self.callables | other.callables,
            self.element_callables | other.element_callables,
            self.runtime_reasons | other.runtime_reasons,
            self.call_reasons | other.call_reasons,
            self.element_runtime_reasons | other.element_runtime_reasons,
            self.element_call_reasons | other.element_call_reasons,
        )

    def element_shape(self) -> "TypeShape":
        return TypeShape(
            direct=self.elements,
            callables=self.element_callables,
            runtime_reasons=self.element_runtime_reasons,
            call_reasons=self.element_call_reasons,
        )


_EMPTY = TypeShape()


def _position(node: ast.AST, *, end: bool = False) -> tuple[int, int]:
    line_name = "end_lineno" if end else "lineno"
    column_name = "end_col_offset" if end else "col_offset"
    return (getattr(node, line_name, 0), getattr(node, column_name, 0))


def _precedes(node: ast.AST, before: ast.AST) -> bool:
    return _position(node, end=True) <= _position(before)


def _merge_shapes(shapes: list[TypeShape]) -> TypeShape:
    result = _EMPTY
    for shape in shapes:
        result = result.merge(shape)
    return result


def _elements_from_shapes(shapes: list[TypeShape]) -> TypeShape:
    return TypeShape(
        runtime_reasons=frozenset({"builtin-target"}),
        elements=frozenset(
            identifier
            for shape in shapes
            for identifier in (*shape.direct, *shape.elements)
        ),
        element_callables=frozenset(
            identifier
            for shape in shapes
            for identifier in (*shape.callables, *shape.element_callables)
        ),
        element_runtime_reasons=frozenset(
            reason
            for shape in shapes
            for reason in (*shape.runtime_reasons, *shape.element_runtime_reasons)
        ),
        element_call_reasons=frozenset(
            reason
            for shape in shapes
            for reason in (*shape.call_reasons, *shape.element_call_reasons)
        ),
    )

"""Repository-local Python call-target resolution façade."""

from __future__ import annotations

import ast

from .bindings import BindingResolver, dotted
from .callgraph_annotations import AnnotationFlow
from .callgraph_callbacks import CallbackFlow
from .callgraph_dispatch import DispatchFlow
from .callgraph_expression import ExpressionFlow
from .callgraph_indexes import build_callgraph_indexes
from .callgraph_scope import ScopeFlow
from .callgraph_shapes import TypeShape
from .model import CallInfo, Facts


class CallGraphResolver(
    AnnotationFlow,
    ExpressionFlow,
    ScopeFlow,
    CallbackFlow,
    DispatchFlow,
):
    def __init__(self, facts: Facts, bindings: BindingResolver) -> None:
        self.facts = facts
        self.bindings = bindings
        self._return_cache: dict[str, TypeShape] = {}
        self._return_active: set[str] = set()
        self._parameter_cache: dict[tuple[str, str], TypeShape] = {}
        self._parameter_active: set[tuple[str, str]] = set()
        self._field_cache: dict[tuple[str, str], TypeShape] = {}
        self._base_cache: dict[str, tuple[str, ...]] = {}
        self._runtime_base_cache: dict[str, frozenset[str]] = {}
        self._mro_cache: dict[str, tuple[str, ...]] = {}
        self._descendant_cache: dict[str, frozenset[str]] = {}
        self._children_by_base: dict[str, tuple[str, ...]] | None = None
        self._decorator_argument_shapes: dict[tuple[str, str], TypeShape] = {}
        self._effective_targets: dict[str, frozenset[str]] = {}
        self._direct_callers: dict[str, tuple[CallInfo, ...]] = {}
        self._indexes = build_callgraph_indexes(facts)
        self._index_decorators()
        self._return_cache.clear()
        self._parameter_cache.clear()
        self._direct_callers = self._index_direct_callers()

    @staticmethod
    def _runtime_reason(reasons: frozenset[str]) -> str | None:
        if not reasons:
            return None
        return next(iter(reasons)) if len(reasons) == 1 else "dynamic-target"

    def resolve_call(self, call: CallInfo) -> tuple[tuple[str, ...], str]:
        reference = dotted(call.callee)
        if reference in {"importlib.import_module", "import_module", "__import__"}:
            return (), "dynamic-target"
        if isinstance(call.callee, ast.Call) and dotted(call.callee.func) == "getattr":
            attribute = call.callee.args[1] if len(call.callee.args) > 1 else None
            if not isinstance(attribute, ast.Constant) or not isinstance(attribute.value, str):
                return (), "dynamic-target"
        targets, reason = self._callable_targets(
            call.callee,
            call.module_name,
            call.class_qname,
            call.scope_id,
            call.expression_node,
            set(),
        )
        return tuple(sorted(targets)), reason

    def _callable_targets(
        self,
        callee: ast.AST,
        module_name: str,
        class_qname: str | None,
        scope_id: str | None,
        before: ast.AST,
        seen: set[tuple[str, str]],
    ) -> tuple[set[str], str]:
        if isinstance(callee, ast.Name):
            if callee.id == "cls" and class_qname:
                class_id = self.facts.symbols.get(class_qname)
                return (set(self._instance_type_ids(class_id)), "") if class_id else (set(), "missing-target")
            local = self._local_shape(callee.id, module_name, class_qname, scope_id, before, seen)
            if local.callables:
                return set(local.callables), ""
            local_reason = self._runtime_reason(local.call_reasons)
            if local_reason:
                return set(), local_reason
            instance_targets: set[str] = set()
            for class_id in local.direct:
                instance_targets.update(self._method_targets(class_id, "__call__"))
            if instance_targets:
                return instance_targets, ""
            local_reason = self._runtime_reason(local.runtime_reasons)
            if local_reason:
                return set(), local_reason
            target_id, reason = self.bindings.resolve_reference(
                module_name, class_qname, callee.id, scope_id
            )
            return (set(self._effective_target_ids(target_id)), "") if target_id else (set(), reason)
        if isinstance(callee, ast.Attribute):
            if isinstance(callee.value, ast.Call) and isinstance(callee.value.func, ast.Name) and callee.value.func.id == "super":
                targets = self._super_method_targets(class_qname, callee.attr)
                if targets:
                    return targets, ""
                base_reason = self._runtime_reason(self._runtime_base_reasons(class_qname))
                return set(), base_reason or "missing-target"
            target_id, reason = self.bindings.resolve_reference(
                module_name, class_qname, dotted(callee), scope_id
            )
            if target_id:
                return set(self._effective_target_ids(target_id)), ""
            attribute_shape = self.expression_shape(
                callee, module_name, class_qname, scope_id, before, seen
            )
            if attribute_shape.callables:
                return set(attribute_shape.callables), ""
            attribute_reason = self._runtime_reason(attribute_shape.call_reasons)
            if attribute_reason:
                return set(), attribute_reason
            receiver = self.expression_shape(
                callee.value, module_name, class_qname, scope_id, before, seen
            )
            targets: set[str] = set()
            for class_id in receiver.direct:
                targets.update(self._method_targets(class_id, callee.attr))
            if targets:
                return targets, ""
            receiver_reason = self._runtime_reason(receiver.runtime_reasons)
            if receiver_reason:
                return set(), receiver_reason
            if receiver.direct:
                inherited_reasons: set[str] = set()
                for class_id in receiver.direct:
                    inherited_reasons.update(
                        self._runtime_base_reasons(self.facts.node_qnames.get(class_id))
                    )
                inherited_reason = self._runtime_reason(frozenset(inherited_reasons))
                return set(), inherited_reason or "missing-target"
            return set(), reason if reason != "missing-target" else "dynamic-target"
        if isinstance(callee, ast.Lambda):
            lambda_id = self.facts.lambda_ids.get((module_name, callee.lineno, callee.col_offset))
            return ({lambda_id}, "") if lambda_id else (set(), "dynamic-target")
        if isinstance(callee, (ast.Call, ast.Subscript, ast.IfExp)):
            returned = self.expression_shape(callee, module_name, class_qname, scope_id, before, seen)
            targets = set(returned.callables)
            for class_id in returned.direct:
                targets.update(self._method_targets(class_id, "__call__"))
            if targets:
                return targets, ""
            returned_reason = self._runtime_reason(
                returned.call_reasons or returned.runtime_reasons
            )
            return set(), returned_reason or "dynamic-target"
        return set(), "dynamic-target"

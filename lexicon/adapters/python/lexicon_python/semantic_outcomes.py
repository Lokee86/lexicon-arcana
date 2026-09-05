"""Emit normalized outcome obligations from proven async call edges."""

from __future__ import annotations

import ast
from typing import Any

from .contract import span
from .model import Facts, FileContext
from .semantic_facts import is_generated_semantic_source

SpanKey = tuple[str, int, int, int, int]


def emit_outcome_facts(facts: Facts, contexts: list[FileContext]) -> None:
    async_ids = {
        info.node_id
        for info in facts.functions.values()
        if isinstance(info.node, ast.AsyncFunctionDef)
    }
    proven_spans = {
        key
        for edge in facts.edges.values()
        if edge.get("relation") == "calls" and edge.get("target") in async_ids
        if (key := _span_key(edge.get("span"))) is not None
    }
    if not proven_spans:
        return
    for context in contexts:
        if context.tree is not None and not is_generated_semantic_source(context.source):
            _OutcomeCollector(facts, context, proven_spans).visit(context.tree)


def _span_key(value: Any) -> SpanKey | None:
    if not isinstance(value, dict):
        return None
    try:
        return (
            str(value["path"]),
            int(value["start_line"]),
            int(value["start_column"]),
            int(value["end_line"]),
            int(value["end_column"]),
        )
    except (KeyError, TypeError, ValueError):
        return None


class _OutcomeCollector(ast.NodeVisitor):
    def __init__(self, facts: Facts, context: FileContext, proven_spans: set[SpanKey]) -> None:
        self.facts = facts
        self.context = context
        self.proven_spans = proven_spans
        self.parents: list[ast.AST] = []

    def visit(self, node: ast.AST) -> None:
        parent = self.parents[-1] if self.parents else None
        if isinstance(node, ast.Call):
            self._emit_call(node, parent)
        self.parents.append(node)
        super().visit(node)
        self.parents.pop()

    def _emit_call(self, node: ast.Call, parent: ast.AST | None) -> None:
        operation_span = span(node, self.context.relative_path, self.context.lines)
        if _span_key(operation_span) not in self.proven_spans:
            return
        location = f"{node.lineno}:{node.col_offset}"
        identity = f"@semantic/outcome-operation/python/{self.context.relative_path}:{location}"
        operation_id = self.facts.add_node(
            "protocol",
            "outcome-operation:python:async",
            self.context.relative_path,
            identity,
            identity=identity,
            record_span=operation_span,
        )
        if isinstance(parent, ast.Expr):
            return
        action_identity = f"{identity}/consume:{location}"
        action_id = self.facts.add_node(
            "protocol",
            "outcome-action:consume",
            self.context.relative_path,
            action_identity,
            identity=action_identity,
            record_span=operation_span,
        )
        self.facts.add_edge(operation_id, action_id, "contains", record_span=operation_span)

"""Emit normalized outcome obligations from proven async call edges."""

from __future__ import annotations

from typing import Any

from .model import Facts

SpanKey = tuple[str, int, int, int, int]


def emit_outcome_facts(facts: Facts) -> None:
    async_ids = {
        info.node_id
        for info in facts.functions.values()
        if info.is_async
    }
    proven_spans = {
        edge.span
        for edge in facts.edges
        if edge.relation == "calls"
        and edge.target in async_ids
        and edge.span is not None
    }
    if not proven_spans:
        return

    for call in facts.calls:
        if not call.outcome_eligible:
            continue
        operation_span = call.record_span
        span_key = _span_key(operation_span)
        if span_key not in proven_spans or operation_span is None:
            continue
        path = span_key[0]
        node = call.expression_node
        location = f"{node.lineno}:{node.col_offset}"
        identity = f"@semantic/outcome-operation/python/{path}:{location}"
        operation_id = facts.add_node(
            "protocol",
            "outcome-operation:python:async",
            path,
            identity,
            identity=identity,
            record_span=operation_span,
        )
        if call.bare_expression:
            continue
        action_identity = f"{identity}/consume:{location}"
        action_id = facts.add_node(
            "protocol",
            "outcome-action:consume",
            path,
            action_identity,
            identity=action_identity,
            record_span=operation_span,
        )
        facts.add_edge(
            operation_id,
            action_id,
            "contains",
            record_span=operation_span,
        )


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

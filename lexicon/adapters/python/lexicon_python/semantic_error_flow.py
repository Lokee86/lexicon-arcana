"""Emit normalized downstream dispositions for semantically empty Python handlers."""

from __future__ import annotations

import ast

from .contract import node_id, span
from .model import Facts, FileContext


FlowEvidence = tuple[str, ast.stmt]


def emit_error_flow_facts(facts: Facts, context: FileContext) -> None:
    if context.tree is None:
        return
    parents = _parent_map(context.tree)
    for handler in ast.walk(context.tree):
        if not isinstance(handler, ast.ExceptHandler) or not _empty_handler(handler):
            continue
        evidence = _downstream_flow(handler, parents)
        if evidence is not None:
            _emit_flow(facts, context, handler, *evidence)


def _parent_map(tree: ast.AST) -> dict[ast.AST, ast.AST]:
    parents: dict[ast.AST, ast.AST] = {}
    for parent in ast.walk(tree):
        for child in ast.iter_child_nodes(parent):
            parents[child] = parent
    return parents


def _empty_handler(handler: ast.ExceptHandler) -> bool:
    return bool(handler.body) and all(isinstance(statement, ast.Pass) for statement in handler.body)


def _downstream_flow(handler: ast.ExceptHandler, parents: dict[ast.AST, ast.AST]) -> FlowEvidence | None:
    owner = parents.get(handler)
    if not isinstance(owner, (ast.Try, ast.TryStar)):
        return None
    suffix = _following_statements(owner, parents)
    if not suffix:
        return None
    first = suffix[0]
    if isinstance(first, ast.Raise):
        return "enclosing-propagation", first
    if isinstance(first, ast.Return):
        return "fallback", first
    if isinstance(first, (ast.Try, ast.TryStar)):
        return "fallback", first
    if isinstance(first, (ast.Assign, ast.AnnAssign, ast.AugAssign)):
        next_statement = suffix[1] if len(suffix) > 1 else None
        if isinstance(next_statement, ast.Return):
            return "fallback", first
    return "continuation", first


def _following_statements(owner: ast.stmt, parents: dict[ast.AST, ast.AST]) -> list[ast.stmt]:
    current: ast.AST = owner
    while True:
        container = parents.get(current)
        if container is None:
            return []
        for _, value in ast.iter_fields(container):
            if not isinstance(value, list) or current not in value:
                continue
            index = value.index(current)
            suffix = [statement for statement in value[index + 1 :] if isinstance(statement, ast.stmt)]
            if suffix:
                return suffix
            break
        if isinstance(container, ast.If):
            current = container
            continue
        return []


def _emit_flow(
    facts: Facts,
    context: FileContext,
    handler: ast.ExceptHandler,
    flow: str,
    evidence: ast.stmt,
) -> None:
    location = f"{handler.lineno}:{handler.col_offset}"
    handler_identity = f"@semantic/error-handler/python/{context.relative_path}:{location}"
    handler_id = node_id("protocol", handler_identity)
    evidence_location = f"{evidence.lineno}:{evidence.col_offset}"
    flow_identity = f"{handler_identity}/flow-{flow}:{evidence_location}"
    flow_span = span(evidence, context.relative_path, context.lines)
    flow_id = facts.add_node(
        "protocol",
        f"error-flow:{flow}",
        context.relative_path,
        flow_identity,
        identity=flow_identity,
        record_span=flow_span,
    )
    facts.add_edge(handler_id, flow_id, "contains", record_span=flow_span)

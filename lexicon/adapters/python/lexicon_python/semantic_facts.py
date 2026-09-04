"""Emit language-neutral semantic capability and error-handling facts."""

from __future__ import annotations

import ast

from .contract import span
from .model import Facts, FileContext

CAPABILITIES = ("control-flow", "error-handling", "calls", "source-spans", "outcome-obligations")
_ACTIONS = ("propagate", "record", "recover")
_RECORDING_TARGETS = frozenset(
    {
        "capture_error",
        "capture_exception",
        "critical",
        "debug",
        "error",
        "exception",
        "info",
        "log",
        "report_error",
        "warn",
        "warning",
    }
)


def emit_semantic_facts(facts: Facts, contexts: list[FileContext]) -> None:
    for context in contexts:
        if context.tree is None:
            continue
        _emit_capabilities(facts, context)
        collector = _HandlerCollector(facts, context)
        collector.visit(context.tree)


def _emit_capabilities(facts: Facts, context: FileContext) -> None:
    identity = f"@semantic/capabilities/python/{context.relative_path}"
    facts.add_node(
        "protocol",
        f"semantic-capabilities:python:{','.join(CAPABILITIES)}",
        context.relative_path,
        identity,
        identity=identity,
    )


class _HandlerCollector(ast.NodeVisitor):
    def __init__(self, facts: Facts, context: FileContext) -> None:
        self.facts = facts
        self.context = context

    def visit_ExceptHandler(self, node: ast.ExceptHandler) -> None:
        location = f"{node.lineno}:{node.col_offset}"
        identity = f"@semantic/error-handler/python/{self.context.relative_path}:{location}"
        handler_id = self.facts.add_node(
            "protocol",
            "error-handler:python",
            self.context.relative_path,
            identity,
            identity=identity,
            record_span=span(node, self.context.relative_path, self.context.lines),
        )
        actions = _ActionCollector()
        for statement in node.body:
            actions.visit(statement)
        for action in _ACTIONS:
            action_node = actions.actions.get(action)
            if action_node is None:
                continue
            action_location = f"{action_node.lineno}:{action_node.col_offset}"
            action_identity = f"{identity}/{action}:{action_location}"
            action_span = span(action_node, self.context.relative_path, self.context.lines)
            action_id = self.facts.add_node(
                "protocol",
                f"error-action:{action}",
                self.context.relative_path,
                action_identity,
                identity=action_identity,
                record_span=action_span,
            )
            self.facts.add_edge(handler_id, action_id, "contains", record_span=action_span)
        self.generic_visit(node)


class _ActionCollector(ast.NodeVisitor):
    def __init__(self) -> None:
        self.actions: dict[str, ast.AST] = {}

    def _record(self, action: str, node: ast.AST) -> None:
        self.actions.setdefault(action, node)

    def visit_Raise(self, node: ast.Raise) -> None:
        self._record("propagate", node)

    def visit_Return(self, node: ast.Return) -> None:
        self._record("recover", node)

    def visit_Call(self, node: ast.Call) -> None:
        action = "record" if _call_leaf(node.func) in _RECORDING_TARGETS else "recover"
        self._record(action, node)
        self.generic_visit(node)

    def visit_Assign(self, node: ast.Assign) -> None:
        self._record("recover", node)
        self.generic_visit(node)

    def visit_AnnAssign(self, node: ast.AnnAssign) -> None:
        self._record("recover", node)
        self.generic_visit(node)

    def visit_AugAssign(self, node: ast.AugAssign) -> None:
        self._record("recover", node)
        self.generic_visit(node)

    def visit_NamedExpr(self, node: ast.NamedExpr) -> None:
        self._record("recover", node)
        self.generic_visit(node)

    def visit_Break(self, node: ast.Break) -> None:
        self._record("recover", node)

    def visit_Continue(self, node: ast.Continue) -> None:
        self._record("recover", node)

    def visit_ExceptHandler(self, node: ast.ExceptHandler) -> None:
        return

    def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
        return

    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef) -> None:
        return

    def visit_Lambda(self, node: ast.Lambda) -> None:
        return

    def visit_ClassDef(self, node: ast.ClassDef) -> None:
        return


def _call_leaf(node: ast.AST) -> str | None:
    if isinstance(node, ast.Name):
        return node.id.lower()
    if isinstance(node, ast.Attribute):
        return node.attr.lower()
    return None

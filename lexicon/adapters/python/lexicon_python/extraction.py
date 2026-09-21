"""AST declaration and relationship evidence collection."""

from __future__ import annotations

import ast

from .contract import expression_text, is_generated_semantic_source, span
from .extraction_declarations import DeclarationFlow
from .extraction_flow import LocalFlow
from .extraction_imports import ImportFlow
from .model import Facts, FileContext


class DeclarationVisitor(
    DeclarationFlow,
    ImportFlow,
    LocalFlow,
    ast.NodeVisitor,
):
    def __init__(self, facts: Facts, context: FileContext) -> None:
        self.facts = facts
        self.context = context
        self.class_stack: list[tuple[str, str]] = []
        self.function_stack: list[tuple[str, str]] = []
        self.lexical_stack: list[tuple[str, str]] = []
        self.owner_stack: list[str] = [context.module_id]
        self.import_index = 0
        self.control_flow_depth = 0
        self.direct_class_statement: ast.stmt | None = None
        self.data_symbols: dict[tuple[str, str], str] = {}
        self.bare_expression_call: ast.Call | None = None
        self.semantic_outcomes_enabled = not is_generated_semantic_source(context.source)

    @property
    def owner_id(self) -> str:
        return self.owner_stack[-1]

    @property
    def class_qname(self) -> str | None:
        return self.class_stack[-1][0] if self.class_stack else None

    @property
    def scope_id(self) -> str:
        return self.owner_id

    def _declare_data_symbol(self, name: str, kind: str = "variable", node: ast.AST | None = None) -> str:
        key = (self.scope_id, name)
        if key in self.data_symbols:
            return self.data_symbols[key]
        owner_name = self.facts.node_qnames.get(self.owner_id, self.context.module_name)
        qualified = f"{owner_name}.{name}"
        record_span = span(node, self.context.relative_path, self.context.source) if node else None
        identifier = self.facts.add_node(kind, name, self.context.relative_path, qualified, identity=f"{self.owner_id}:{name}", record_span=record_span)
        self.data_symbols[key] = identifier
        self.facts.add_edge(self.owner_id, identifier, "defines", record_span=record_span)
        return identifier

    def _resolve_data_symbol(self, name: str) -> str | None:
        owner = self.owner_id
        while owner:
            symbol = self.data_symbols.get((owner, name))
            if symbol:
                return symbol
            owner = self.facts.scope_parents.get(owner, "")
        return None

    def _emit_dataflow(self, node: ast.AST, relation: str, name: str) -> None:
        symbol = self._resolve_data_symbol(name)
        if symbol:
            self.facts.add_dataflow_edge(self.owner_id, symbol, relation, record_span=span(node, self.context.relative_path, self.context.source))

    def _attributes(self, node: ast.AST) -> dict[str, object]:
        decorators = sorted(expression_text(item, self.context.source) for item in getattr(node, "decorator_list", []))
        attributes: dict[str, object] = {}
        if decorators:
            attributes["decorators"] = decorators
        if isinstance(node, ast.AsyncFunctionDef):
            attributes["async"] = True
        return attributes

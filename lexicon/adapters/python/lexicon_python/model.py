"""Mutable fact graph and extraction context models."""

from __future__ import annotations

import ast
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .contract import node_id
from .fact_records import EdgeFact, NodeFact, UnresolvedFact


@dataclass(slots=True)
class ImportInfo:
    module_name: str
    owner_id: str
    node_id: str
    expression: str
    binding: str | None
    target_module: str | None = None
    target_name: str | None = None
    relative_level: int = 0
    star: bool = False
    is_package: bool = False


@dataclass(slots=True)
class InheritanceInfo:
    source_id: str
    module_name: str
    class_qname: str
    base: ast.expr
    expression: str
    record_span: dict[str, Any] | None


@dataclass(slots=True)
class FunctionInfo:
    module_name: str
    qname: str
    node_id: str
    class_qname: str | None
    arguments: ast.arguments
    decorators: tuple[ast.expr, ...]
    return_expressions: tuple[ast.expr, ...]
    parameters: dict[str, ast.expr | None]
    return_annotation: ast.expr | None
    is_lambda: bool
    is_async: bool


@dataclass(slots=True)
class ClassInfo:
    module_name: str
    qname: str
    node_id: str
    bases: tuple[ast.expr, ...]


@dataclass(slots=True)
class CallInfo:
    module_name: str
    owner_id: str
    class_qname: str | None
    scope_id: str | None
    expression_node: ast.Call
    callee: ast.AST
    expression: str
    record_span: dict[str, Any] | None
    bare_expression: bool
    outcome_eligible: bool


@dataclass(slots=True)
class LocalAssignmentInfo:
    module_name: str
    scope_id: str
    class_qname: str | None
    name: str
    assignment_node: ast.AST
    value: ast.expr | None
    annotation: ast.expr | None
    branch_dependent: bool
    direct_class_field: bool = False

    @property
    def constructor(self) -> ast.Call | None:
        return self.value if isinstance(self.value, ast.Call) else None


@dataclass(slots=True)
class LoopBindingInfo:
    module_name: str
    scope_id: str
    class_qname: str | None
    name: str
    loop_node: ast.AST
    iterable: ast.expr
    branch_dependent: bool
    element_index: int | None = None


@dataclass(slots=True)
class FileContext:
    root: Path
    path: Path
    relative_path: str
    module_name: str
    source: str
    tree: ast.AST | None
    file_id: str
    module_id: str
    data: bytes
    parse_error: str | None = None


@dataclass(slots=True)
class RepositorySnapshot:
    root: Path
    repository: str
    directories: list[Path]
    contexts: list[FileContext]


@dataclass(slots=True)
class Facts:
    repository: str
    nodes: dict[str, NodeFact] = field(default_factory=dict)
    edges: dict[EdgeFact, None] = field(default_factory=dict)
    unresolved: dict[UnresolvedFact, None] = field(default_factory=dict)
    modules: dict[str, str] = field(default_factory=dict)
    symbols: dict[str, str] = field(default_factory=dict)
    node_qnames: dict[str, str] = field(default_factory=dict)
    imports: list[ImportInfo] = field(default_factory=list)
    inheritances: list[InheritanceInfo] = field(default_factory=list)
    functions: dict[str, FunctionInfo] = field(default_factory=dict)
    classes: dict[str, ClassInfo] = field(default_factory=dict)
    # Stable source coordinates, not process-local AST identities.
    lambda_ids: dict[tuple[str, int, int], str] = field(default_factory=dict)
    calls: list[CallInfo] = field(default_factory=list)
    local_assignments: list[LocalAssignmentInfo] = field(default_factory=list)
    loop_bindings: list[LoopBindingInfo] = field(default_factory=list)
    module_bindings: dict[tuple[str, str], tuple[str | None, str]] = field(default_factory=dict)
    scope_bindings: dict[tuple[str, str], tuple[str | None, str]] = field(default_factory=dict)
    scope_parents: dict[str, str] = field(default_factory=dict)
    dataflow_edges: set[tuple[str, str, str]] = field(default_factory=set)

    def add_node(
        self,
        kind: str,
        name: str,
        path: str,
        qualified_name: str,
        *,
        identity: str | None = None,
        record_span: dict[str, Any] | None = None,
        attributes: dict[str, Any] | None = None,
        file_content_id: str | None = None,
    ) -> str:
        identifier = node_id(kind, identity if identity is not None else qualified_name)
        self.nodes[identifier] = NodeFact.create(
            identifier,
            kind,
            name,
            path,
            qualified_name,
            file_content_id,
            attributes,
            record_span,
        )
        self.node_qnames[identifier] = qualified_name
        return identifier

    def add_edge(
        self,
        source: str,
        target: str,
        relation: str,
        *,
        record_span: dict[str, Any] | None = None,
        attributes: dict[str, Any] | None = None,
    ) -> None:
        self.edges.setdefault(
            EdgeFact.create(
                source,
                target,
                relation,
                record_span,
                attributes,
            ),
            None,
        )

    def add_dataflow_edge(
        self,
        source: str,
        target: str,
        relation: str,
        *,
        record_span: dict[str, Any] | None = None,
    ) -> None:
        key = (source, target, relation)
        if key in self.dataflow_edges:
            return
        self.dataflow_edges.add(key)
        self.add_edge(source, target, relation, record_span=record_span)

    def add_unresolved(
        self,
        source: str,
        relation: str,
        expression: str,
        reason: str,
        *,
        record_span: dict[str, Any] | None = None,
        candidate_name: str | None = None,
    ) -> None:
        self.unresolved.setdefault(
            UnresolvedFact.create(
                source,
                relation,
                expression,
                reason,
                record_span,
                candidate_name,
            ),
            None,
        )

    def release_analysis_state(self) -> None:
        """Release extraction/resolution state after durable facts are complete."""
        for mapping in (
            self.modules,
            self.symbols,
            self.node_qnames,
            self.functions,
            self.classes,
            self.lambda_ids,
            self.module_bindings,
            self.scope_bindings,
            self.scope_parents,
        ):
            mapping.clear()
        for items in (
            self.imports,
            self.inheritances,
            self.calls,
            self.local_assignments,
            self.loop_bindings,
        ):
            items.clear()
        self.dataflow_edges.clear()

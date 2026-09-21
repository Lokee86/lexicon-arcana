"""Mutable fact graph and extraction context models."""

from __future__ import annotations

import ast
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .contract import node_id


@dataclass
class ImportInfo:
    module_name: str
    owner_id: str
    node_id: str
    statement: ast.AST
    expression: str
    binding: str | None
    target_module: str | None = None
    target_name: str | None = None
    relative_level: int = 0
    star: bool = False
    is_package: bool = False


@dataclass
class InheritanceInfo:
    source_id: str
    module_name: str
    class_qname: str
    base: ast.expr
    source: str
    path: str
    lines: list[str]


@dataclass
class FunctionInfo:
    module_name: str
    qname: str
    node_id: str
    class_qname: str | None
    node: ast.FunctionDef | ast.AsyncFunctionDef | ast.Lambda
    parameters: dict[str, ast.expr | None]
    return_annotation: ast.expr | None


@dataclass
class ClassInfo:
    module_name: str
    qname: str
    node_id: str
    node: ast.ClassDef


@dataclass
class CallInfo:
    module_name: str
    owner_id: str
    class_qname: str | None
    scope_id: str | None
    expression_node: ast.Call
    callee: ast.AST


@dataclass
class LocalAssignmentInfo:
    module_name: str
    scope_id: str
    class_qname: str | None
    name: str
    assignment_node: ast.AST
    value: ast.expr | None
    annotation: ast.expr | None
    branch_dependent: bool

    @property
    def constructor(self) -> ast.Call | None:
        return self.value if isinstance(self.value, ast.Call) else None


@dataclass
class LoopBindingInfo:
    module_name: str
    scope_id: str
    class_qname: str | None
    name: str
    loop_node: ast.AST
    iterable: ast.expr
    branch_dependent: bool
    element_index: int | None = None


@dataclass
class FileContext:
    root: Path
    path: Path
    relative_path: str
    module_name: str
    source: str
    lines: list[str]
    tree: ast.AST | None
    file_id: str
    module_id: str
    data: bytes
    parse_error: str | None = None


@dataclass
class RepositorySnapshot:
    root: Path
    repository: str
    directories: list[Path]
    contexts: list[FileContext]


@dataclass
class Facts:
    repository: str
    nodes: dict[str, dict[str, Any]] = field(default_factory=dict)
    edges: dict[str, dict[str, Any]] = field(default_factory=dict)
    unresolved: dict[str, dict[str, Any]] = field(default_factory=dict)
    modules: dict[str, str] = field(default_factory=dict)
    symbols: dict[str, str] = field(default_factory=dict)
    symbol_kinds: dict[str, str] = field(default_factory=dict)
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
        record: dict[str, Any] = {
            "record": "node",
            "id": identifier,
            "kind": kind,
            "name": name,
            "path": path,
            "qualified_name": qualified_name,
        }
        if file_content_id is not None:
            record["content_id"] = file_content_id
        if attributes:
            record["attributes"] = attributes
        if record_span is not None:
            record["span"] = record_span
        self.nodes[identifier] = record
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
        record: dict[str, Any] = {
            "record": "edge",
            "relation": relation,
            "source": source,
            "target": target,
        }
        if attributes:
            record["attributes"] = attributes
        if record_span is not None:
            record["span"] = record_span
        key = json.dumps(record, sort_keys=True, separators=(",", ":"))
        self.edges[key] = record

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
        record: dict[str, Any] = {
            "record": "unresolved",
            "relation": relation,
            "source": source,
            "expression": expression,
            "reason": reason,
        }
        if candidate_name:
            record["candidate_name"] = candidate_name
        if record_span is not None:
            record["span"] = record_span
        key = json.dumps(record, sort_keys=True, separators=(",", ":"))
        self.unresolved[key] = record

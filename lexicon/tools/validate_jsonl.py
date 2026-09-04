#!/usr/bin/env python3
"""Validate structural and incremental invariants of Lexicon facts v1."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path, PurePosixPath
from typing import Any

from semantic_contract import validate_semantic_links, validate_semantic_protocol_node

ID_PATTERN = re.compile(r"^sha256:[0-9a-f]{64}$")
RECORD_ORDER = {"node": 0, "edge": 1, "unresolved": 2}
NODE_KINDS = {
    "repository",
    "directory",
    "file",
    "module",
    "namespace",
    "symbol",
    "type",
    "interface",
    "protocol",
    "trait",
    "function",
    "method",
    "constructor",
    "field",
    "variable",
    "constant",
    "signal",
    "parameter",
    "import",
    "export",
    "test",
}
RELATIONS = {
    "contains",
    "defines",
    "imports",
    "calls",
    "possible-calls",
    "passes-to",
    "converts-to",
    "references",
    "extends",
    "implements",
    "uses-trait",
    "overrides",
    "reads",
    "writes",
    "annotates",
    "includes",
    "depends-on",
    "tests",
    "documents",
    "generates",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def validate_path(value: Any, label: str) -> str:
    require(isinstance(value, str) and bool(value), f"{label} must be a non-empty path")
    require("\\" not in value, f"{label} must use forward slashes")
    path = PurePosixPath(value)
    require(not path.is_absolute(), f"{label} must be repository-relative")
    require(".." not in path.parts and "." not in path.parts, f"{label} is not normalized")
    return value


def validate_string_array(header: dict[str, Any], field: str) -> list[str]:
    value = header.get(field)
    require(isinstance(value, list), f"header.{field} must be an array")
    require(all(isinstance(item, str) for item in value), f"header.{field} must contain strings")
    paths = [validate_path(item, f"header.{field}") for item in value]
    require(paths == sorted(set(paths)), f"header.{field} must be sorted and unique")
    return paths


def validate_span(record: dict[str, Any], line: int) -> None:
    span = record.get("span")
    if span is None:
        return
    require(isinstance(span, dict), f"line {line}: span must be an object")
    validate_path(span.get("path"), f"line {line}: span.path")
    for field in ("start_line", "start_column", "end_line", "end_column"):
        require(isinstance(span.get(field), int) and span[field] >= 1, f"line {line}: span.{field} must be positive")
    require(
        (span["end_line"], span["end_column"]) >= (span["start_line"], span["start_column"]),
        f"line {line}: span end precedes start",
    )


def span_key(record: dict[str, Any]) -> tuple[Any, ...]:
    span = record.get("span") or {}
    return (
        span.get("path", ""),
        span.get("start_line", 0),
        span.get("start_column", 0),
        span.get("end_line", 0),
        span.get("end_column", 0),
    )


def sort_key(record: dict[str, Any]) -> tuple[Any, ...]:
    kind = record["record"]
    if kind == "node":
        return (0, record["id"], record["kind"], record["path"], record["qualified_name"])
    if kind == "edge":
        return (1, record["source"], record["target"], record["relation"], *span_key(record))
    return (
        2,
        record["source"],
        record["relation"],
        record["expression"],
        record["reason"],
        *span_key(record),
    )


def record_owner(record: dict[str, Any], nodes: dict[str, dict[str, Any]], line: int) -> str | None:
    owner = record.get("owner")
    if owner is not None:
        return validate_path(owner, f"line {line}: owner")
    span = record.get("span")
    if span is not None:
        return validate_path(span.get("path"), f"line {line}: span.path")
    if record["record"] == "node" and record["kind"] == "file":
        return validate_path(record["path"], f"line {line}: node.path")
    if record["record"] in {"edge", "unresolved"}:
        source = nodes.get(record["source"])
        if source is not None:
            return record_owner(source, nodes, line)
    return None


def validate(path: Path) -> None:
    lines = path.read_text(encoding="utf-8").splitlines()
    require(bool(lines), "fact stream is empty")
    records = [json.loads(line) for line in lines]
    require(all(isinstance(record, dict) for record in records), "every line must be a JSON object")
    header = records[0]
    require(header.get("record") == "lexicon", "first record must be the Lexicon header")
    require(header.get("schema_version") == 1, "schema_version must be 1")
    for field in ("adapter_version", "language", "repository"):
        require(isinstance(header.get(field), str) and bool(header[field]), f"header.{field} is required")

    mode = header.get("mode", "full")
    require(mode in {"full", "incremental"}, "header.mode must be full or incremental")
    changed_files: list[str] = []
    removed_files: list[str] = []
    if mode == "incremental":
        changed_files = validate_string_array(header, "changed_files")
        removed_files = validate_string_array(header, "removed_files")
        require(not set(changed_files) & set(removed_files), "changed_files and removed_files must be disjoint")
    else:
        require("changed_files" not in header and "removed_files" not in header, "full streams cannot declare incremental scope")

    facts = records[1:]
    nodes: dict[str, dict[str, Any]] = {}
    for index, record in enumerate(facts, start=2):
        record_kind = record.get("record")
        require(record_kind in RECORD_ORDER, f"line {index}: invalid record kind")
        validate_span(record, index)
        if record_kind == "node":
            for field in ("id", "kind", "name", "path", "qualified_name"):
                require(field in record, f"line {index}: node.{field} is required")
            require(ID_PATTERN.match(record["id"]) is not None, f"line {index}: invalid node id")
            require(record["kind"] in NODE_KINDS, f"line {index}: invalid node kind")
            validate_path(record["path"], f"line {index}: node.path")
            require(isinstance(record["name"], str), f"line {index}: node.name must be a string")
            require(isinstance(record["qualified_name"], str), f"line {index}: node.qualified_name must be a string")
            require(record["id"] not in nodes, f"line {index}: duplicate node id")
            nodes[record["id"]] = record
            content_id = record.get("content_id")
            require(content_id is None or ID_PATTERN.match(content_id) is not None, f"line {index}: invalid content id")
            validate_semantic_protocol_node(record, index, require)
        elif record_kind == "edge":
            for field in ("source", "target", "relation"):
                require(field in record, f"line {index}: edge.{field} is required")
            require(ID_PATTERN.match(record["source"]) is not None, f"line {index}: invalid edge source")
            require(ID_PATTERN.match(record["target"]) is not None, f"line {index}: invalid edge target")
            require(record["relation"] in RELATIONS, f"line {index}: invalid edge relation")
        else:
            for field in ("source", "relation", "expression", "reason"):
                require(field in record, f"line {index}: unresolved.{field} is required")
            require(ID_PATTERN.match(record["source"]) is not None, f"line {index}: invalid unresolved source")
            require(record["relation"] in RELATIONS, f"line {index}: invalid unresolved relation")
            require(isinstance(record["expression"], str), f"line {index}: unresolved.expression must be a string")
            require(isinstance(record["reason"], str), f"line {index}: unresolved.reason must be a string")

    if mode == "full":
        for index, record in enumerate(facts, start=2):
            if record["record"] == "edge":
                require(record["source"] in nodes, f"line {index}: unknown edge source")
                require(record["target"] in nodes, f"line {index}: unknown edge target")
            elif record["record"] == "unresolved":
                require(record["source"] in nodes, f"line {index}: unknown unresolved source")
    else:
        changed = set(changed_files)
        removed = set(removed_files)
        for index, record in enumerate(facts, start=2):
            owner = record_owner(record, nodes, index)
            require(owner is not None, f"line {index}: incremental record has no resolvable owner")
            require(owner in changed, f"line {index}: owner is outside changed_files")
            require(owner not in removed, f"line {index}: removed file cannot emit replacement records")

    validate_semantic_links(facts, nodes, require)
    require(facts == sorted(facts, key=sort_key), "fact records are not canonically sorted")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("facts", type=Path)
    args = parser.parse_args()
    try:
        validate(args.facts)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

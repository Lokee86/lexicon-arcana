"""Canonical Lexicon record ordering and JSONL emission."""

from __future__ import annotations

import json
import sys
from collections.abc import Iterable, Iterator
from pathlib import Path
from typing import Any, TextIO

from .contract import LANGUAGE, SCHEMA_VERSION
from .fact_records import EdgeFact, NodeFact, SpanRecord, UnresolvedFact
from .model import Facts


def _span_sort_key(value: SpanRecord | None) -> tuple[Any, ...]:
    if value is None:
        return ("", 0, 0, 0, 0)
    return value


def _node_sort_key(record: NodeFact) -> tuple[Any, ...]:
    return (
        0,
        record.identifier,
        record.kind,
        record.path,
        record.qualified_name,
    )


def _edge_sort_key(record: EdgeFact) -> tuple[Any, ...]:
    return (
        1,
        record.source,
        record.target,
        record.relation,
        *_span_sort_key(record.span),
    )


def _unresolved_sort_key(record: UnresolvedFact) -> tuple[Any, ...]:
    return (
        2,
        record.source,
        record.relation,
        record.expression,
        record.reason,
        *_span_sort_key(record.span),
    )


def emit_records(
    facts: Facts,
    adapter_version: str,
    changed_files: list[str] | None = None,
    removed_files: list[str] | None = None,
) -> list[dict[str, Any]]:
    return list(iter_records(facts, adapter_version, changed_files, removed_files))


def iter_records(
    facts: Facts,
    adapter_version: str,
    changed_files: list[str] | None = None,
    removed_files: list[str] | None = None,
) -> Iterator[dict[str, Any]]:
    incremental = changed_files is not None or removed_files is not None
    selected = {_normalize(path) for path in changed_files or []}
    header = {
        "record": "lexicon",
        "schema_version": SCHEMA_VERSION,
        "adapter_version": adapter_version,
        "language": LANGUAGE,
        "repository": facts.repository,
    }
    if incremental:
        header["mode"] = "incremental"
        header["changed_files"] = sorted(selected)
        header["removed_files"] = sorted(_normalize(path) for path in removed_files or [])
        header["shared_complete"] = True
    yield header

    nodes = sorted(facts.nodes.values(), key=_node_sort_key)
    owners = (
        {record.identifier: record.direct_owner() for record in nodes}
        if incremental
        else {}
    )
    for fact in nodes:
        record = fact.to_record()
        if not incremental or _include(record, owners, selected):
            yield record
    del nodes

    edges = sorted(facts.edges, key=_edge_sort_key)
    for fact in edges:
        record = fact.to_record()
        if not incremental or _include(record, owners, selected):
            yield record
    del edges

    unresolved = sorted(facts.unresolved, key=_unresolved_sort_key)
    for fact in unresolved:
        record = fact.to_record()
        if not incremental or _include(record, owners, selected):
            yield record


def _normalize(path: str) -> str:
    return Path(path).as_posix()


def _direct_owner(record: dict[str, Any]) -> str:
    owner = record.get("owner")
    if isinstance(owner, str) and owner:
        return _normalize(owner)
    span = record.get("span")
    if isinstance(span, dict) and isinstance(span.get("path"), str):
        return _normalize(span["path"])
    if record.get("record") == "node" and record.get("kind") == "file":
        path = record.get("path")
        return _normalize(path) if isinstance(path, str) else ""
    return ""


def _include(
    record: dict[str, Any],
    owners: dict[str, str],
    selected: set[str],
) -> bool:
    owner = _direct_owner(record)
    if not owner:
        source = record.get("source")
        if isinstance(source, str):
            owner = owners.get(source, "")
    return not owner or owner in selected


def write_records(records: Iterable[dict[str, Any]], output: Path) -> None:
    if str(output) == "-":
        if hasattr(sys.stdout, "reconfigure"):
            sys.stdout.reconfigure(encoding="utf-8", errors="strict", newline="\n")
        _write_stream(records, sys.stdout)
        return
    destination = output.expanduser()
    destination.parent.mkdir(parents=True, exist_ok=True)
    with destination.open("w", encoding="utf-8", newline="\n") as stream:
        _write_stream(records, stream)


def _write_stream(records: Iterable[dict[str, Any]], stream: TextIO) -> None:
    for record in records:
        stream.write(
            json.dumps(
                record,
                ensure_ascii=False,
                sort_keys=True,
                separators=(",", ":"),
            )
        )
        stream.write("\n")

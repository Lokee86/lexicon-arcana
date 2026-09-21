"""Compact durable fact records used by the Python adapter."""

from __future__ import annotations

import json
from dataclasses import dataclass
from typing import Any

SpanKey = tuple[str, int, int, int, int]
SpanRecord = SpanKey


def span_from_dict(value: dict[str, Any] | None) -> SpanKey | None:
    if value is None:
        return None
    return (
        str(value.get("path", "")),
        int(value.get("start_line", 0)),
        int(value.get("start_column", 0)),
        int(value.get("end_line", 0)),
        int(value.get("end_column", 0)),
    )


def span_to_dict(value: SpanKey | None) -> dict[str, Any] | None:
    if value is None:
        return None
    path, start_line, start_column, end_line, end_column = value
    return {
        "end_column": end_column,
        "end_line": end_line,
        "path": path,
        "start_column": start_column,
        "start_line": start_line,
    }


def _encode_attributes(attributes: dict[str, Any] | None) -> str:
    if not attributes:
        return ""
    return json.dumps(attributes, sort_keys=True, separators=(",", ":"))


def _decode_attributes(value: str) -> dict[str, Any] | None:
    if not value:
        return None
    decoded = json.loads(value)
    return decoded if isinstance(decoded, dict) else None


@dataclass(slots=True)
class NodeFact:
    identifier: str
    kind: str
    name: str
    path: str
    qualified_name: str
    content_id: str | None
    attributes: dict[str, Any] | None
    span: SpanKey | None

    @classmethod
    def create(
        cls,
        identifier: str,
        kind: str,
        name: str,
        path: str,
        qualified_name: str,
        content_id: str | None,
        attributes: dict[str, Any] | None,
        record_span: dict[str, Any] | None,
    ) -> "NodeFact":
        return cls(
            identifier=identifier,
            kind=kind,
            name=name,
            path=path,
            qualified_name=qualified_name,
            content_id=content_id,
            attributes=attributes if attributes else None,
            span=span_from_dict(record_span),
        )

    def direct_owner(self) -> str:
        if self.span is not None:
            return self.span[0]
        if self.kind == "file":
            return self.path
        return ""

    def to_record(self) -> dict[str, Any]:
        record: dict[str, Any] = {
            "record": "node",
            "id": self.identifier,
            "kind": self.kind,
            "name": self.name,
            "path": self.path,
            "qualified_name": self.qualified_name,
        }
        if self.content_id is not None:
            record["content_id"] = self.content_id
        if self.attributes is not None:
            record["attributes"] = self.attributes
        record_span = span_to_dict(self.span)
        if record_span is not None:
            record["span"] = record_span
        return record


@dataclass(frozen=True, slots=True)
class EdgeFact:
    source: str
    target: str
    relation: str
    span: SpanKey | None
    attributes_json: str = ""

    @classmethod
    def create(
        cls,
        source: str,
        target: str,
        relation: str,
        record_span: dict[str, Any] | None,
        attributes: dict[str, Any] | None,
    ) -> "EdgeFact":
        return cls(
            source=source,
            target=target,
            relation=relation,
            span=span_from_dict(record_span),
            attributes_json=_encode_attributes(attributes),
        )

    def to_record(self) -> dict[str, Any]:
        record: dict[str, Any] = {
            "record": "edge",
            "relation": self.relation,
            "source": self.source,
            "target": self.target,
        }
        attributes = _decode_attributes(self.attributes_json)
        if attributes is not None:
            record["attributes"] = attributes
        record_span = span_to_dict(self.span)
        if record_span is not None:
            record["span"] = record_span
        return record


@dataclass(frozen=True, slots=True)
class UnresolvedFact:
    source: str
    relation: str
    expression: str
    reason: str
    candidate_name: str
    span: SpanKey | None

    @classmethod
    def create(
        cls,
        source: str,
        relation: str,
        expression: str,
        reason: str,
        record_span: dict[str, Any] | None,
        candidate_name: str | None,
    ) -> "UnresolvedFact":
        return cls(
            source=source,
            relation=relation,
            expression=expression,
            reason=reason,
            candidate_name=candidate_name or "",
            span=span_from_dict(record_span),
        )

    def to_record(self) -> dict[str, Any]:
        record: dict[str, Any] = {
            "record": "unresolved",
            "relation": self.relation,
            "source": self.source,
            "expression": self.expression,
            "reason": self.reason,
        }
        if self.candidate_name:
            record["candidate_name"] = self.candidate_name
        record_span = span_to_dict(self.span)
        if record_span is not None:
            record["span"] = record_span
        return record

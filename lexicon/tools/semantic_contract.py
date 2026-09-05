"""Validation helpers for Lexicon semantic fact contract v1."""

from __future__ import annotations

import re
from typing import Any, Callable


CAPABILITY_ORDER = (
    "control-flow",
    "error-handling",
    "calls",
    "source-spans",
    "outcome-obligations",
)
ERROR_ACTIONS = {"propagate", "record", "recover"}
ERROR_FLOWS = {"fallback", "enclosing-propagation", "intentional-suppression", "continuation"}
OUTCOME_KINDS = {"fallible", "async"}
OUTCOME_ACTIONS = {"consume"}
_LANGUAGE_PATTERN = re.compile(r"^[a-z][a-z0-9+-]*$")

Require = Callable[[bool, str], None]


def validate_semantic_protocol_node(record: dict[str, Any], line: int, require: Require) -> None:
    if record.get("kind") != "protocol":
        return
    name = record.get("name", "")
    qualified_name = record.get("qualified_name", "")
    if name.startswith("semantic-capabilities:"):
        _validate_capabilities(record, line, require)
    elif name.startswith("error-handler:"):
        language = name.split(":", 1)[1]
        require(_valid_language(language), f"line {line}: invalid error-handler language")
        require(
            qualified_name.startswith(f"@semantic/error-handler/{language}/{record['path']}:"),
            f"line {line}: invalid error-handler qualified_name",
        )
        require(record.get("span") is not None, f"line {line}: error-handler requires a source span")
    elif name.startswith("error-action:"):
        action = name.split(":", 1)[1]
        require(action in ERROR_ACTIONS, f"line {line}: unknown semantic error action")
        _require_action_identity(record, line, require, "@semantic/error-handler/", action)
    elif name.startswith("error-flow:"):
        flow = name.split(":", 1)[1]
        require(flow in ERROR_FLOWS, f"line {line}: unknown semantic error flow")
        _require_action_identity(record, line, require, "@semantic/error-handler/", f"flow-{flow}")
    elif name.startswith("outcome-operation:"):
        parts = name.split(":", 2)
        require(len(parts) == 3, f"line {line}: invalid outcome operation name")
        language, outcome = parts[1], parts[2]
        require(_valid_language(language), f"line {line}: invalid outcome-operation language")
        require(outcome in OUTCOME_KINDS, f"line {line}: unknown outcome obligation")
        require(
            qualified_name.startswith(f"@semantic/outcome-operation/{language}/{record['path']}:"),
            f"line {line}: invalid outcome-operation qualified_name",
        )
        require(record.get("span") is not None, f"line {line}: outcome-operation requires a source span")
    elif name.startswith("outcome-action:"):
        action = name.split(":", 1)[1]
        require(action in OUTCOME_ACTIONS, f"line {line}: unknown semantic outcome action")
        _require_action_identity(record, line, require, "@semantic/outcome-operation/", action)


def _validate_capabilities(record: dict[str, Any], line: int, require: Require) -> None:
    parts = record["name"].split(":", 2)
    require(len(parts) == 3, f"line {line}: invalid semantic capability node name")
    language, raw_capabilities = parts[1], parts[2]
    require(_valid_language(language), f"line {line}: invalid semantic language")
    capabilities = raw_capabilities.split(",") if raw_capabilities else []
    order = {value: index for index, value in enumerate(CAPABILITY_ORDER)}
    require(bool(capabilities), f"line {line}: semantic capability set is empty")
    require(len(capabilities) == len(set(capabilities)), f"line {line}: duplicate semantic capability")
    require(all(value in order for value in capabilities), f"line {line}: unknown semantic capability")
    require(
        capabilities == sorted(capabilities, key=order.__getitem__),
        f"line {line}: semantic capabilities are not canonically ordered",
    )
    require(
        record["qualified_name"] == f"@semantic/capabilities/{language}/{record['path']}",
        f"line {line}: invalid semantic capability qualified_name",
    )


def _require_action_identity(record: dict[str, Any], line: int, require: Require, prefix: str, action: str) -> None:
    require(
        record["qualified_name"].startswith(prefix) and f"/{action}:" in record["qualified_name"],
        f"line {line}: invalid semantic action qualified_name",
    )
    require(record.get("span") is not None, f"line {line}: semantic action requires a source span")


def _valid_language(value: str) -> bool:
    return _LANGUAGE_PATTERN.match(value) is not None


def validate_semantic_links(
    records: list[dict[str, Any]],
    nodes: dict[str, dict[str, Any]],
    require: Require,
) -> None:
    _validate_action_links(records, nodes, require, "error-action:", "error-handler:", "error")
    _validate_action_links(records, nodes, require, "error-flow:", "error-handler:", "error flow")
    _validate_action_links(records, nodes, require, "outcome-action:", "outcome-operation:", "outcome")


def _validate_action_links(
    records: list[dict[str, Any]],
    nodes: dict[str, dict[str, Any]],
    require: Require,
    action_prefix: str,
    owner_prefix: str,
    label: str,
) -> None:
    actions = {
        node_id: node
        for node_id, node in nodes.items()
        if node.get("kind") == "protocol" and str(node.get("name", "")).startswith(action_prefix)
    }
    if not actions:
        return
    linked: set[str] = set()
    for record in records:
        if record.get("record") != "edge" or record.get("target") not in actions:
            continue
        source = nodes.get(record.get("source", ""))
        target = actions[record["target"]]
        require(record.get("relation") == "contains", f"semantic {label} actions require contains edges")
        require(
            source is not None and str(source.get("name", "")).startswith(owner_prefix),
            f"semantic {label} actions must be contained by {label} owners",
        )
        require(source.get("path") == target.get("path"), f"semantic {label} owner/action paths must match")
        linked.add(record["target"])
    require(linked == set(actions), f"every semantic {label} action must be contained by its owner")

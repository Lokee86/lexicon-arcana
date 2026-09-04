"""Validation helpers for Lexicon semantic fact contract v1."""

from __future__ import annotations

import re
from typing import Any, Callable


CAPABILITY_ORDER = (
    "control-flow",
    "error-handling",
    "calls",
    "source-spans",
)
ERROR_ACTIONS = {"propagate", "record", "recover"}
_LANGUAGE_PATTERN = re.compile(r"^[a-z][a-z0-9+-]*$")

Require = Callable[[bool, str], None]


def validate_semantic_protocol_node(record: dict[str, Any], line: int, require: Require) -> None:
    if record.get("kind") != "protocol":
        return
    name = record.get("name", "")
    qualified_name = record.get("qualified_name", "")
    if name.startswith("semantic-capabilities:"):
        parts = name.split(":", 2)
        require(len(parts) == 3, f"line {line}: invalid semantic capability node name")
        language, raw_capabilities = parts[1], parts[2]
        require(_LANGUAGE_PATTERN.match(language) is not None, f"line {line}: invalid semantic language")
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
            qualified_name == f"@semantic/capabilities/{language}/{record['path']}",
            f"line {line}: invalid semantic capability qualified_name",
        )
    elif name.startswith("error-handler:"):
        language = name.split(":", 1)[1]
        require(_LANGUAGE_PATTERN.match(language) is not None, f"line {line}: invalid error-handler language")
        require(
            qualified_name.startswith(f"@semantic/error-handler/{language}/{record['path']}:"),
            f"line {line}: invalid error-handler qualified_name",
        )
        require(record.get("span") is not None, f"line {line}: error-handler requires a source span")
    elif name.startswith("error-action:"):
        action = name.split(":", 1)[1]
        require(action in ERROR_ACTIONS, f"line {line}: unknown semantic error action")
        require(
            qualified_name.startswith("@semantic/error-handler/") and f"/{action}:" in qualified_name,
            f"line {line}: invalid error-action qualified_name",
        )
        require(record.get("span") is not None, f"line {line}: error-action requires a source span")


def validate_semantic_links(
    records: list[dict[str, Any]],
    nodes: dict[str, dict[str, Any]],
    require: Require,
) -> None:
    actions = {
        node_id: node
        for node_id, node in nodes.items()
        if node.get("kind") == "protocol" and str(node.get("name", "")).startswith("error-action:")
    }
    if not actions:
        return
    linked: set[str] = set()
    for record in records:
        if record.get("record") != "edge" or record.get("target") not in actions:
            continue
        source = nodes.get(record.get("source", ""))
        target = actions[record["target"]]
        require(record.get("relation") == "contains", "semantic error actions require contains edges")
        require(
            source is not None and str(source.get("name", "")).startswith("error-handler:"),
            "semantic error actions must be contained by error handlers",
        )
        require(source.get("path") == target.get("path"), "semantic handler/action paths must match")
        linked.add(record["target"])
    require(linked == set(actions), "every semantic error action must be contained by an error handler")

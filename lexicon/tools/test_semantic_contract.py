from __future__ import annotations

import unittest

from semantic_contract import validate_semantic_links, validate_semantic_protocol_node


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


class SemanticContractTest(unittest.TestCase):
    def test_valid_capability_handler_and_action_contract(self) -> None:
        capability = {
            "kind": "protocol",
            "name": "semantic-capabilities:python:control-flow,error-handling,calls,source-spans",
            "path": "src/app.py",
            "qualified_name": "@semantic/capabilities/python/src/app.py",
        }
        handler = {
            "id": "handler",
            "kind": "protocol",
            "name": "error-handler:python",
            "path": "src/app.py",
            "qualified_name": "@semantic/error-handler/python/src/app.py:4:1",
            "span": {"path": "src/app.py"},
        }
        action = {
            "id": "action",
            "kind": "protocol",
            "name": "error-action:record",
            "path": "src/app.py",
            "qualified_name": "@semantic/error-handler/python/src/app.py:4:1/record:5:5",
            "span": {"path": "src/app.py"},
        }
        for line, node in enumerate((capability, handler, action), start=1):
            validate_semantic_protocol_node(node, line, require)
        validate_semantic_links(
            [{"record": "edge", "relation": "contains", "source": "handler", "target": "action"}],
            {"handler": handler, "action": action},
            require,
        )

    def test_valid_outcome_obligation_and_consumption_contract(self) -> None:
        operation = {
            "id": "operation",
            "kind": "protocol",
            "name": "outcome-operation:python:async",
            "path": "src/app.py",
            "qualified_name": "@semantic/outcome-operation/python/src/app.py:8:5",
            "span": {"path": "src/app.py"},
        }
        action = {
            "id": "consume",
            "kind": "protocol",
            "name": "outcome-action:consume",
            "path": "src/app.py",
            "qualified_name": "@semantic/outcome-operation/python/src/app.py:8:5/consume:8:5",
            "span": {"path": "src/app.py"},
        }
        validate_semantic_protocol_node(operation, 1, require)
        validate_semantic_protocol_node(action, 2, require)
        validate_semantic_links(
            [{"record": "edge", "relation": "contains", "source": "operation", "target": "consume"}],
            {"operation": operation, "consume": action},
            require,
        )

    def test_rejects_unknown_or_noncanonical_capabilities(self) -> None:
        for name in (
            "semantic-capabilities:python:control-flow,unknown",
            "semantic-capabilities:python:calls,control-flow",
            "semantic-capabilities:python:calls,calls",
        ):
            with self.subTest(name=name), self.assertRaises(ValueError):
                validate_semantic_protocol_node(
                    {
                        "kind": "protocol",
                        "name": name,
                        "path": "src/app.py",
                        "qualified_name": "@semantic/capabilities/python/src/app.py",
                    },
                    1,
                    require,
                )

    def test_rejects_uncontained_or_misowned_error_actions(self) -> None:
        action = {
            "id": "action",
            "kind": "protocol",
            "name": "error-action:recover",
            "path": "src/app.py",
            "qualified_name": "@semantic/error-handler/python/src/app.py:4:1/recover:5:5",
            "span": {"path": "src/app.py"},
        }
        with self.assertRaises(ValueError):
            validate_semantic_links([], {"action": action}, require)

        non_handler = {
            "id": "source",
            "kind": "function",
            "name": "work",
            "path": "src/app.py",
        }
        with self.assertRaises(ValueError):
            validate_semantic_links(
                [{"record": "edge", "relation": "contains", "source": "source", "target": "action"}],
                {"source": non_handler, "action": action},
                require,
            )


if __name__ == "__main__":
    unittest.main()

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path


ADAPTER_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ADAPTER_ROOT))

from lexicon_python.adapter import build_facts


class PythonSemanticFactsTest(unittest.TestCase):
    def test_error_handlers_emit_shared_semantic_contract(self) -> None:
        with tempfile.TemporaryDirectory() as tempdir:
            repo = Path(tempdir) / "fixture"
            repo.mkdir()
            (repo / "sample.py").write_text(
                "import logging\n\n"
                "def fallback():\n"
                "    return None\n\n"
                "def swallowed():\n"
                "    try:\n"
                "        fallback()\n"
                "    except Exception:\n"
                "        pass\n\n"
                "def propagated():\n"
                "    try:\n"
                "        fallback()\n"
                "    except Exception:\n"
                "        raise\n\n"
                "def recorded():\n"
                "    try:\n"
                "        fallback()\n"
                "    except Exception as error:\n"
                "        logging.error(\"failed: %s\", error)\n\n"
                "def recovered():\n"
                "    try:\n"
                "        fallback()\n"
                "    except Exception:\n"
                "        fallback()\n\n"
                "def nested_only():\n"
                "    try:\n"
                "        fallback()\n"
                "    except Exception:\n"
                "        def later():\n"
                "            logging.error(\"later\")\n"
                "        pass\n",
                encoding="utf-8",
            )

            records = build_facts(repo)
            nodes = [record for record in records if record["record"] == "node"]
            edges = [record for record in records if record["record"] == "edge"]
            capabilities = [
                node
                for node in nodes
                if node["kind"] == "protocol"
                and str(node["name"]).startswith("semantic-capabilities:python:")
            ]
            handlers = [
                node
                for node in nodes
                if node["kind"] == "protocol" and node["name"] == "error-handler:python"
            ]
            actions = [
                node
                for node in nodes
                if node["kind"] == "protocol" and str(node["name"]).startswith("error-action:")
            ]

            self.assertEqual(len(capabilities), 1)
            self.assertEqual(
                capabilities[0]["name"],
                "semantic-capabilities:python:control-flow,error-handling,calls,source-spans,outcome-obligations",
            )
            self.assertEqual(len(handlers), 5)
            self.assertEqual(
                {node["name"] for node in actions},
                {"error-action:propagate", "error-action:record", "error-action:recover"},
            )
            action_ids = {node["id"] for node in actions}
            self.assertEqual(
                len(
                    [
                        edge
                        for edge in edges
                        if edge["relation"] == "contains" and edge["target"] in action_ids
                    ]
                ),
                3,
            )
            swallowed_handlers = [
                handler
                for handler in handlers
                if not any(
                    edge["relation"] == "contains"
                    and edge["source"] == handler["id"]
                    and edge["target"] in action_ids
                    for edge in edges
                )
            ]
            self.assertEqual(len(swallowed_handlers), 2)
            self.assertTrue(
                all(
                    str(node["qualified_name"]).startswith("@semantic/")
                    for node in capabilities + handlers + actions
                )
            )


if __name__ == "__main__":
    unittest.main()

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path


ADAPTER_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ADAPTER_ROOT))

from lexicon_python.adapter import build_facts


class PythonSemanticOutcomeTest(unittest.TestCase):
    def test_async_calls_emit_consumed_and_unobserved_outcomes(self) -> None:
        with tempfile.TemporaryDirectory() as tempdir:
            repo = Path(tempdir) / "fixture"
            repo.mkdir()
            (repo / "sample.py").write_text(
                "async def fetch():\n"
                "    return 1\n\n"
                "async def use():\n"
                "    fetch()\n"
                "    await fetch()\n"
                "    value = fetch()\n"
                "    return fetch()\n",
                encoding="utf-8",
            )

            records = build_facts(repo)
            nodes = [record for record in records if record["record"] == "node"]
            edges = [record for record in records if record["record"] == "edge"]
            operations = [
                node
                for node in nodes
                if node["kind"] == "protocol" and node["name"] == "outcome-operation:python:async"
            ]
            actions = [
                node
                for node in nodes
                if node["kind"] == "protocol" and node["name"] == "outcome-action:consume"
            ]
            action_ids = {node["id"] for node in actions}
            consumed_sources = {
                edge["source"]
                for edge in edges
                if edge["relation"] == "contains" and edge["target"] in action_ids
            }

            self.assertEqual(len(operations), 4)
            self.assertEqual(len(actions), 3)
            self.assertEqual(
                len([operation for operation in operations if operation["id"] not in consumed_sources]),
                1,
            )

    def test_async_method_name_is_outside_static_proof_boundary(self) -> None:
        with tempfile.TemporaryDirectory() as tempdir:
            repo = Path(tempdir) / "fixture"
            repo.mkdir()
            (repo / "sample.py").write_text(
                "class Worker:\n"
                "    async def run(self):\n"
                "        return 1\n\n"
                "def use():\n"
                "    run()\n",
                encoding="utf-8",
            )
            records = build_facts(repo)
            self.assertFalse(
                any(
                    record["record"] == "node"
                    and record.get("name") == "outcome-operation:python:async"
                    for record in records
                )
            )


if __name__ == "__main__":
    unittest.main()

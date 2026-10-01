"""Protect calibration's materialized graph-source invariant."""

import json
from pathlib import Path
import tempfile
import unittest

from c_family_phase2_metrics import summarize_facts


class SourceOwnershipTests(unittest.TestCase):
    def summarize(self, source, kind="edge"):
        root = Path(__file__).resolve().parents[1]
        with tempfile.TemporaryDirectory(prefix=".tmp-cfamily-metrics-", dir=root) as raw:
            repository = Path(raw)
            records = [
                {"record": "node", "id": "owned", "kind": "function"},
                {
                    "record": kind,
                    "source": source,
                    "target": "external-target",
                    "relation": "calls",
                    "reason": "external",
                },
            ]
            facts = repository / "facts.jsonl"
            facts.write_text("\n".join(map(json.dumps, records)), encoding="utf-8")
            return summarize_facts(repository, facts)

    def test_materialized_source_can_resolve_external_target(self):
        self.assertEqual(self.summarize("owned")["edges"], 1)

    def test_context_identity_cannot_be_an_edge_source(self):
        with self.assertRaisesRegex(RuntimeError, "lack materialized nodes"):
            self.summarize("context-only")

    def test_context_identity_cannot_be_an_unresolved_source(self):
        with self.assertRaisesRegex(RuntimeError, "lack materialized nodes"):
            self.summarize("context-only", "unresolved")


if __name__ == "__main__":
    unittest.main()

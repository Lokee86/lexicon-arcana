"""Mutation tests for the separate, fail-closed Phase 5 repair gate."""
from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from incremental_phase5_repair_gate import inspect

ROOT = Path(__file__).parent
FILES = [
    "incremental-phase5-acp-after-2026-10-01.json",
    "incremental-phase5-shared-repair-package-2026-10-01.json",
    "incremental-phase5-shared-repair-oracle-2026-10-01.json",
    "incremental-phase5-agent-cold-indexed-2026-10-01.json",
]


class RepairGates(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.reports = [json.loads((ROOT / file).read_text(encoding="utf-8"))
                       for file in FILES]

    def test_recorded_fixtures_pass(self) -> None:
        self.assertEqual(inspect(*self.reports), [])

    def test_corrupt_fact_hash_fails(self) -> None:
        reports = copy.deepcopy(self.reports)
        reports[1]["steps"]["edit_once"]["facts"]["records_sha256"] = "different"
        self.assertTrue(any("parity" in error for error in inspect(*reports)))

    def test_missing_oracle_fails(self) -> None:
        reports = copy.deepcopy(self.reports)
        reports[2]["edit_twice"]["matches_independent_full"] = False
        self.assertTrue(any("independent" in error for error in inspect(*reports)))

    def test_unnecessary_full_retry_fails(self) -> None:
        reports = copy.deepcopy(self.reports)
        reports[1]["steps"]["edit_once"]["metrics"].append(
            {"stage": "scan.adapter.python.full"})
        self.assertTrue(any("retry" in error for error in inspect(*reports)))

    def test_incomplete_agent_fails(self) -> None:
        reports = copy.deepcopy(self.reports)
        reports[3]["steps"]["init_full"]["timed_out"] = True
        self.assertTrue(any("incomplete" in error for error in inspect(*reports)))

    def test_revision_and_missing_shards_fail(self) -> None:
        reports = copy.deepcopy(self.reports)
        reports[1]["source_revision"] = "bad"
        reports[3]["steps"]["init_full"]["metrics"] = []
        errors = inspect(*reports)
        self.assertTrue(any("revision" in e for e in errors))
        self.assertTrue(any("shard" in e for e in errors))


if __name__ == "__main__":
    unittest.main()

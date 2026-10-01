"""Mutation tests for the diagnostic cold-pair evidence gate."""
import copy
import json
import unittest
from pathlib import Path

from incremental_phase5_cold_pair_gate import inspect

ROOT = Path(__file__).parent
FILES = (
    "incremental-phase5-agent-baseline-pair-2026-10-01.json",
    "incremental-phase5-agent-optimized-pair-2026-10-01.json",
    "incremental-phase5-agent-detectors-profile-2026-10-01.json",
)


class ColdPairGates(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.reports = [json.loads((ROOT / path).read_text(encoding="utf-8"))
                       for path in FILES]

    def mutate(self, name: int, target) -> list[str]:
        copies = copy.deepcopy(self.reports)
        target(copies[name])
        return inspect(*copies)

    def test_grounded_pair(self) -> None:
        self.assertEqual(inspect(*self.reports), [])

    def test_wrong_revision_fails(self) -> None:
        self.assertTrue(self.mutate(1, lambda r: r.update(source_revision="other")))

    def test_censored_before_fails(self) -> None:
        self.assertTrue(self.mutate(0, lambda r: r["steps"]["init_full"]
                                    .update(timed_out=True)))

    def test_fact_mismatch_fails(self) -> None:
        self.assertTrue(self.mutate(1, lambda r: r["steps"]["init_full"]["facts"]
                                    .update(records_sha256="wrong")))

    def test_unimproved_stage_fails(self) -> None:
        self.assertTrue(self.mutate(1, lambda r: next(
            x for x in r["steps"]["init_full"]["metrics"]
            if x["stage"] == "interstack.contract_detection").update(
                elapsed_ms=999999)))

    def test_changed_interstack_cardinality_fails(self) -> None:
        self.assertTrue(self.mutate(1, lambda r: next(
            x for x in r["steps"]["init_full"]["metrics"]
            if x["stage"] == "interstack.linking").update(edges=0)))

    def test_missing_detector_profile_fails(self) -> None:
        self.assertTrue(self.mutate(2, lambda r: r["steps"]["init_full"]
                                    .update(metrics=[])))


if __name__ == "__main__":
    unittest.main()

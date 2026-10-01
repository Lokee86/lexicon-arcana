"""Phase 5 gate must not accept missing or mismatched measurements."""
import copy
import unittest

from incremental_phase5_compare import compare


def step(seconds: float = 1, digest: str = "same") -> dict:
    return {
        "exit_code": 0, "timed_out": False, "wall_seconds": seconds,
        "peak_sampled_rss_bytes": 123456,
        "metrics": [],
        "facts": {"records_sha256": digest, "fact_record_count": 10},
        "snapshot_id": digest,
    }


def run() -> dict:
    stages = (
        "init_full", "unchanged", "edit_once", "edit_twice",
        "dirty_mirror", "edit_ten", "add_file", "remove_file", "rename_file",
    )
    result = {
        "source_revision": "pinned",
        "fixture": {"fixture": "same"},
        "provenance": {"machine": "same"},
        "steps": {name: step() for name in stages},
    }
    full = result["steps"]["init_full"]
    full["metrics"] = [{"stage": "scan.adapter.python.full", "elapsed_ms": 60000}]
    for name in ("edit_once", "edit_twice"):
        result["steps"][name]["metrics"] = [
            {"stage": "scan.source_inventory",
             "byte_equal_skips": 0, "copied_files": 1},
            {"stage": "scan.dependency_index_query", "fact_object_reads": 0},
            {"stage": "scan.dependency_index_delta", "fact_object_reads": 0},
        ]
    result["steps"]["recover"] = step()
    result["steps"]["post_recovery_unchanged"] = step()
    return result


class AcceptanceTests(unittest.TestCase):
    def test_complete_comparable_evidence_passes(self):
        before = run()
        after = run()
        before["steps"]["edit_ten"]["wall_seconds"] = 15
        after["steps"]["edit_ten"]["wall_seconds"] = 12
        after["steps"]["unchanged"]["wall_seconds"] = 4
        after["steps"]["edit_once"]["wall_seconds"] = 8
        result = compare(before, after)
        self.assertEqual(result["status"], "pass")
        self.assertTrue(all(value == "pass" for value in result["gates"].values()))

    def test_censored_full_run_never_proves_acceptance(self):
        before = run()
        after = run()
        after["steps"]["init_full"]["timed_out"] = True
        after["steps"]["init_full"]["metrics"] = []
        result = compare(before, after)
        self.assertEqual(result["gates"]["one_edit_under_20pct_full_adapter"],
                         "inconclusive")
        self.assertNotEqual(result["status"], "pass")

    def test_semantic_mismatch_is_a_hard_failure(self):
        before = run()
        after = run()
        after["steps"]["edit_once"]["facts"]["records_sha256"] = "different"
        result = compare(before, after)
        self.assertEqual(result["gates"]["semantic_parity"], "fail")
        self.assertIn("edit_once exported semantic facts differ", result["mismatches"])

    def test_unfinished_recovery_does_not_pass(self):
        before = run()
        after = run()
        del after["steps"]["post_recovery_unchanged"]
        result = compare(before, after)
        self.assertEqual(result["gates"]["after_cancellation_recovery"],
                         "inconclusive")
        self.assertNotEqual(result["status"], "pass")

    def test_different_hardware_is_not_comparable(self):
        before = run()
        after = copy.deepcopy(before)
        after["provenance"]["machine"] = "other"
        result = compare(before, after)
        self.assertEqual(result["gates"]["comparable_inputs"], "fail")


if __name__ == "__main__":
    unittest.main()

"""Unit gates for production-path incremental work-cardinality evidence."""
import unittest

from incremental_phase4_gate import validate_edit, validate_run


def record(files: int, edited: bool) -> dict:
    metrics = [{
        "stage": "scan.source_inventory",
        "discovered_files": files,
        "indexed_skips": files - int(edited),
        "byte_equal_skips": 0,
        "copied_files": int(edited),
    }]
    if edited:
        metrics.extend([
            {"stage": "scan.dependency_index_query",
             "fact_object_reads": 0, "loaded_partitions": 1,
             "roots": 1, "emit_files": 1},
            {"stage": "scan.dependency_index_delta",
             "fact_object_reads": 0, "loaded_partitions": 8,
             "changed_files": 1},
            {"stage": "scan.topology_safety_check",
             "fact_object_reads": 1, "selected_files": 1,
             "full_required": 0},
        ])
    return {"exit_code": 0, "timed_out": False, "metrics": metrics,
            "snapshot_id": "sha256:1"}


class BoundedScanTests(unittest.TestCase):
    def test_bounded_unchanged_and_repeated_edits_pass(self) -> None:
        for count in (61, 1001):
            run = {"steps": {
                "init_full": {"snapshot_id": "sha256:1"},
                "unchanged": record(count, False),
                "edit_once": record(count, True),
                "edit_twice": record(count, True),
            }}
            self.assertEqual(validate_run(run), [])

    def test_source_byte_comparisons_rejected(self) -> None:
        changed = record(1001, True)
        changed["metrics"][0]["byte_equal_skips"] = 1000
        self.assertTrue(validate_edit(changed, 1001))

    def test_repository_wide_bootstrap_rejected(self) -> None:
        changed = record(1001, True)
        changed["metrics"].append({
            "stage": "scan.dependency_bootstrap", "fact_object_reads": 1002,
        })
        self.assertTrue(validate_edit(changed, 1001))

    def test_unrelated_fact_reads_rejected(self) -> None:
        changed = record(1001, True)
        changed["metrics"][1]["fact_object_reads"] = 1002
        self.assertTrue(validate_edit(changed, 1001))
        changed = record(1001, True)
        changed["metrics"][3]["fact_object_reads"] = 1002
        self.assertTrue(validate_edit(changed, 1001))

    def test_missing_metrics_and_large_scopes_rejected(self) -> None:
        changed = record(1001, True)
        changed["metrics"].pop()
        self.assertTrue(validate_edit(changed, 1001))
        changed = record(1001, True)
        changed["metrics"][2]["loaded_partitions"] = 48
        self.assertTrue(validate_edit(changed, 1001))

    def test_unchanged_snapshot_mutation_rejected(self) -> None:
        run = {"steps": {
            "init_full": {"snapshot_id": "sha256:0"},
            "unchanged": record(61, False),
            "edit_once": record(61, True),
            "edit_twice": record(61, True),
        }}
        self.assertTrue(validate_run(run))


if __name__ == "__main__":
    unittest.main()

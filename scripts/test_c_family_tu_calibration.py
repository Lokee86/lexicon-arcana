"""Unit tests for direct-TU calibration result gates."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import c_family_tu_calibration as calibration


class CalibrationMetricsTests(unittest.TestCase):
    def test_extracts_required_direct_tu_metrics(self) -> None:
        stages = {
            "c-family.clang.execution": {
                "discovered_owned_files": 17,
                "primary_real_parse_units": 4,
                "primary_synthetic_parse_units": 2,
                "explicit_header_compile_units": 1,
                "orphan_fallback_units": 1,
                "active_clang_lanes": 5,
                "completed_tus": 7,
                "claimed_owned_files": 17,
                "discarded_duplicate_file_observations": 12,
                "completed_orphan_tus": 1,
                "claimed_orphan_files": 1,
                "discarded_duplicate_orphan_observations": 0,
            },
            "c-family.clang.observation_emission": {
                "framed_transport_bytes": 4096,
                "observed_files": 17,
                "peak_helper_rss_bytes": 8192,
                "transport_bytes": 4096,
                "peak_rss_bytes": 8192,
            },
            "c-family.clang.frontend_work": {"elapsed_ms": 23.5},
        }

        self.assertEqual(
            calibration.architecture_metrics(stages),
            {
                "discovered_owned_files": 17,
                "primary_real_parse_units": 4,
                "primary_synthetic_parse_units": 2,
                "explicit_header_compile_units": 1,
                "orphan_fallback_tus": 1,
                "active_clang_lanes": 5,
                "completed_tus": 7,
                "claimed_owned_files": 17,
                "discarded_duplicate_file_observations": 12,
                "completed_orphan_tus": 1,
                "claimed_orphan_files": 1,
                "discarded_duplicate_orphan_observations": 0,
                "framed_transport_bytes": 4096,
                "transport_file_frames": 17,
                "peak_helper_rss_bytes": 8192,
                "transport_bytes": 4096,
                "peak_rss_bytes": 8192,
                "frontend_wall_ms": 23.5,
            },
        )

    def test_rejects_planner_era_or_incomplete_metrics(self) -> None:
        with self.assertRaisesRegex(RuntimeError, "direct-TU calibration metrics"):
            calibration.architecture_metrics({"c-family.clang.frontend_work": {"elapsed_ms": 1}})

    def test_cold_warm_and_worker_hashes_must_match(self) -> None:
        calibration.require_same_fact_hash("canonical", "canonical", "workers=1")
        with self.assertRaisesRegex(RuntimeError, "worker-count 4 canonical fact hash changed"):
            calibration.require_same_fact_hash("canonical", "changed", "worker-count 4")


class GitGateTests(unittest.TestCase):
    def test_git_requires_completed_codebase_memory_within_both_limits(self) -> None:
        calibration.HARD_CUT.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=calibration.HARD_CUT) as raw:
            results = Path(raw)
            (results / "codebase-memory.json").write_text(
                json.dumps(
                    {
                        "result": {
                            "cold": {
                                "completed": True,
                                "wall_ms": calibration.CBM_MAX_COLD_MS,
                                "peak_process_tree_rss_bytes": calibration.CBM_MAX_COLD_RSS_BYTES,
                                "architecture_metrics": {
                                    "discovered_owned_files": 23,
                                    "claimed_owned_files": 23,
                                    "transport_file_frames": 23,
                                },
                            },
                            "warm": {
                                "completed": True,
                                "architecture_metrics": {
                                    "discovered_owned_files": 23,
                                    "claimed_owned_files": 23,
                                    "transport_file_frames": 23,
                                },
                            },
                        }
                    }
                ),
                encoding="utf-8",
            )
            self.assertEqual(
                calibration.require_codebase_memory_gate(results)["case"],
                "codebase-memory",
            )

            record = json.loads((results / "codebase-memory.json").read_text(encoding="utf-8"))
            record["result"]["cold"]["wall_ms"] += 1
            (results / "codebase-memory.json").write_text(json.dumps(record), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "cold wall"):
                calibration.require_codebase_memory_gate(results)

            record["result"]["cold"]["wall_ms"] = calibration.CBM_MAX_COLD_MS
            record["result"]["warm"]["architecture_metrics"]["transport_file_frames"] = 22
            (results / "codebase-memory.json").write_text(json.dumps(record), encoding="utf-8")
            with self.assertRaisesRegex(RuntimeError, "exactly one file frame"):
                calibration.require_codebase_memory_gate(results)


class PrecedingCaseTests(unittest.TestCase):
    def test_rejects_a_preceding_worker_matrix_failure(self) -> None:
        calibration.HARD_CUT.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=calibration.HARD_CUT) as raw:
            results = Path(raw)
            provenance = {"adapter_eval_sha256": "a", "clang_helper_sha256": "b"}
            (results / "multilang-regression.json").write_text(
                json.dumps({"completed": True, "provenance": provenance}),
                encoding="utf-8",
            )
            (results / "leveldb.json").write_text(
                json.dumps(
                    {
                        "schema": "lexicon.c-family.tu-calibration.v2",
                        "provenance": provenance,
                        "result": {
                            "gate_passed": True,
                            "concurrency_required": True,
                            "cold": {"completed": True, "canonical_fact_sha256": "same"},
                            "warm": {"completed": True, "canonical_fact_sha256": "same"},
                            "concurrency": {
                                "1": {"completed": True, "canonical_fact_sha256": "same"},
                                "2": {"completed": True, "canonical_fact_sha256": "same"},
                                "4": {"completed": False, "canonical_fact_sha256": None},
                            },
                        },
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(RuntimeError, "worker-count 4 run did not complete"):
                calibration.require_preceding_cases("nlohmann-json", results, provenance)


if __name__ == "__main__":
    unittest.main()

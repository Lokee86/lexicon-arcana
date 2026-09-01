from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from run_agent_benchmark import CONDITIONS, DEFAULT_CONDITIONS, initialize_summary


class BenchmarkSummaryTests(unittest.TestCase):
    def test_component_ablation_is_supported_but_not_default(self) -> None:
        self.assertIn("lexicon-arcana", CONDITIONS)
        self.assertNotIn("lexicon-arcana", DEFAULT_CONDITIONS)

    def test_existing_tasks_are_preserved_across_selected_runs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            suite = output / "tasks.json"
            existing = {
                "schema": "grimoire.agent-benchmark.v2",
                "task_suite": str(suite),
                "model": "model",
                "provider": "provider",
                "conditions": ["plain"],
                "parallel_within_task": False,
                "sequential_tasks": True,
                "started_at": "initial",
                "tasks": {"first": {"valid": True}},
            }
            (output / "summary.json").write_text(json.dumps(existing), encoding="utf-8")

            existing["provenance"] = {"schema": "test", "hash": "same"}
            (output / "summary.json").write_text(json.dumps(existing), encoding="utf-8")
            summary = initialize_summary(
                output,
                task_suite=suite,
                model="model",
                provider="provider",
                conditions=("cbm", "grimoire"),
                provenance=existing["provenance"],
            )

            self.assertEqual(summary["tasks"], existing["tasks"])
            self.assertEqual(summary["conditions"], ["plain", "cbm", "grimoire"])
            self.assertEqual(summary["started_at"], "initial")
            self.assertIn("last_run_started_at", summary)

    def test_incompatible_existing_summary_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            suite = output / "tasks.json"
            existing = {
                "schema": "grimoire.agent-benchmark.v2",
                "task_suite": str(suite),
                "model": "other-model",
                "provider": "provider",
                "tasks": {},
            }
            (output / "summary.json").write_text(json.dumps(existing), encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "incompatible model"):
                initialize_summary(
                    output,
                    task_suite=suite,
                    model="model",
                    provider="provider",
                    conditions=("plain",),
                    provenance={"schema": "test"},
                )

    def test_changed_provenance_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            suite = output / "tasks.json"
            existing = {
                "schema": "grimoire.agent-benchmark.v2",
                "task_suite": str(suite),
                "model": "model",
                "provider": "provider",
                "provenance": {"schema": "test", "hash": "old"},
                "tasks": {},
            }
            (output / "summary.json").write_text(json.dumps(existing), encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "incompatible provenance"):
                initialize_summary(
                    output,
                    task_suite=suite,
                    model="model",
                    provider="provider",
                    conditions=("plain",),
                    provenance={"schema": "test", "hash": "new"},
                )


if __name__ == "__main__":
    unittest.main()

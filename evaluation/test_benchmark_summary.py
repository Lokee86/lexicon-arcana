from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from run_agent_benchmark import CONDITIONS, DEFAULT_CONDITIONS, initialize_summary


class BenchmarkSummaryTests(unittest.TestCase):
    def test_plain_and_lexicon_arcana_are_the_default_conditions(self) -> None:
        self.assertEqual(DEFAULT_CONDITIONS, ("plain", "lexicon-arcana"))
        self.assertIn("cbm", CONDITIONS)
        self.assertNotIn("grimoire", CONDITIONS)

    def test_existing_tasks_are_preserved_across_selected_runs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            suite = output / "tasks.json"
            existing = {
                "schema": "lexicon-arcana.agent-benchmark.v2",
                "task_suite": str(suite),
                "model": "model",
                "provider": "provider",
                "conditions": ["plain"],
                "parallel_within_task": False,
                "sequential_tasks": True,
                "started_at": "initial",
                "tasks": {"first": {"valid": True}},
                "provenance": {"schema": "test", "hash": "same"},
            }
            (output / "summary.json").write_text(json.dumps(existing), encoding="utf-8")
            summary = initialize_summary(
                output,
                task_suite=suite,
                model="model",
                provider="provider",
                conditions=("lexicon-arcana", "cbm"),
                provenance=existing["provenance"],
            )

            self.assertEqual(summary["tasks"], existing["tasks"])
            self.assertEqual(summary["conditions"], ["plain", "lexicon-arcana", "cbm"])
            self.assertEqual(summary["started_at"], "initial")
            self.assertIn("last_run_started_at", summary)

    def test_incompatible_existing_summary_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            suite = output / "tasks.json"
            existing = {
                "schema": "lexicon-arcana.agent-benchmark.v2",
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
                "schema": "lexicon-arcana.agent-benchmark.v2",
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

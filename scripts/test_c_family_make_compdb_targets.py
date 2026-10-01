"""Tests for prioritized Make compilation contexts and CBM capture wiring."""

from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import c_family_make_compdb as compdb
import c_family_tu_calibration as calibration


class FallbackTargetTests(unittest.TestCase):
    def capture(self, repository, outputs):
        destination = repository / "compile_commands.json"
        arguments = [
            "c_family_make_compdb.py", "--repository", str(repository),
            "--target", "cbm", "--make-arg=CC=clang",
        ]
        for target in ("test-runner", "repro-runner")[:len(outputs) - 1]:
            arguments.extend(["--fallback-target", target])
        with (
            patch("sys.argv", arguments),
            patch.object(compdb.subprocess, "run", side_effect=outputs) as run,
            patch("builtins.print"),
        ):
            compdb.main()
        return destination, run

    def completed(self, output="", returncode=0, stderr=""):
        return subprocess.CompletedProcess(
            ["make"], returncode=returncode, stdout=output, stderr=stderr
        )

    def test_primary_and_first_fallback_own_shared_sources(self):
        outputs = [
            self.completed("clang -DPRODUCTION=1 -c src/shared.c\n"),
            self.completed(
                "clang -DTEST=1 -Itests -c src/shared.c\n"
                "clang -DTEST=1 -Itests -c tests/shared.c\n"
                "clang -DTEST_OTHER=1 -Itests -c tests/shared.c\n"
            ),
            self.completed(
                "clang -DREPRO=1 -c src/shared.c\n"
                "clang -DREPRO=1 -c tests/shared.c\n"
                "clang -DREPRO=1 -Itests/repro -c tests/repro/only.c\n"
            ),
        ]
        with tempfile.TemporaryDirectory() as raw:
            repository = Path(raw).resolve()
            destination, run = self.capture(repository, outputs)
            records = json.loads(destination.read_text(encoding="utf-8"))
            self.assertEqual(len(records), 4)
            self.assertIn("-DPRODUCTION=1", records[0]["command"])
            self.assertIn("-DTEST=1", records[1]["command"])
            self.assertIn("-DTEST_OTHER=1", records[2]["command"])
            self.assertIn("-DREPRO=1", records[3]["command"])
            self.assertEqual(records[3]["file"], str(repository / "tests/repro/only.c"))
            self.assertEqual(
                [call.args[0][6] for call in run.call_args_list],
                ["cbm", "test-runner", "repro-runner"],
            )
            for call in run.call_args_list:
                self.assertEqual(call.args[0][1:4], ["-n", "-B", "-j1"])
                self.assertEqual(call.args[0][-1], "CC=clang")
            original = destination.read_bytes()
            self.capture(repository, outputs)
            self.assertEqual(destination.read_bytes(), original)

    def test_failed_fallback_does_not_replace_existing_database(self):
        with tempfile.TemporaryDirectory() as raw:
            repository = Path(raw).resolve()
            destination = repository / "compile_commands.json"
            destination.write_text("existing database", encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "test-runner failed"):
                self.capture(repository, [
                    self.completed("clang -c src/shared.c\n"),
                    self.completed(returncode=2, stderr="missing target"),
                ])
            self.assertEqual(destination.read_text(encoding="utf-8"), "existing database")

    def test_empty_fallback_fails_without_publishing_partial_capture(self):
        with tempfile.TemporaryDirectory() as raw:
            repository = Path(raw).resolve()
            with self.assertRaisesRegex(SystemExit, "test-runner produced no"):
                self.capture(repository, [
                    self.completed("clang -c src/shared.c\n"), self.completed(),
                ])
            self.assertFalse((repository / "compile_commands.json").exists())

    def test_cbm_calibration_requests_production_then_test_and_repro(self):
        with tempfile.TemporaryDirectory() as raw:
            repository = Path(raw).resolve()
            with (
                patch.object(calibration, "HARD_CUT", repository / "build"),
                patch.object(calibration, "checked_run") as run,
            ):
                calibration.prepare_compilation_database("codebase-memory", repository)
            command = run.call_args.args[0]
            self.assertEqual(command[command.index("--target") + 1], "cbm")
            self.assertEqual(
                [command[i + 1] for i, value in enumerate(command)
                 if value == "--fallback-target"],
                ["build/c/test-runner", "build/c/test-repro-runner"],
            )


if __name__ == "__main__":
    unittest.main()

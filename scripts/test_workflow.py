#!/usr/bin/env python3
"""Deterministic smoke checks for the Lexicon + Arcana release workflow."""

from __future__ import annotations

import hashlib
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

import install as bundle_installer
import workflow


def run_smoke() -> None:
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(WorkflowSmokeTests)
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    if not result.wasSuccessful():
        raise RuntimeError("workflow smoke checks failed")


class WorkflowSmokeTests(unittest.TestCase):
    def test_windows_package_and_install_layout(self) -> None:
        with tempfile.TemporaryDirectory(prefix="lexicon-arcana-workflow-") as temporary:
            root = Path(temporary)
            build = root / "build"
            (build / "bin").mkdir(parents=True)
            (build / "adapters" / "python").mkdir(parents=True)
            (build / "adapters" / "python" / "adapter.py").write_text("pass\n", encoding="utf-8")
            (build / "adapters" / "go").mkdir()
            (build / "adapters" / "go" / "lexicon-go.exe").write_bytes(b"adapter")
            for name in ("lexicon.exe", "arcana.exe"):
                (build / "bin" / name).write_bytes(name.encode())

            release_root = workflow.package_artifacts(
                build, root / "dist", "1.2.3", platform_name="Windows", machine="AMD64"
            )
            checksum_text = (release_root / "SHA256SUMS.txt").read_bytes()
            self.assertNotIn(b"\r", checksum_text)
            archives = sorted(release_root.glob("*.zip"))
            self.assertEqual(len(archives), 3)
            for line in checksum_text.decode().splitlines():
                digest, _, name = line.partition("  ")
                self.assertEqual(digest, hashlib.sha256((release_root / name).read_bytes()).hexdigest())

            combined = release_root / "lexicon-arcana-bundle-1.2.3-windows-x86_64.zip"
            with zipfile.ZipFile(combined) as archive:
                names = archive.namelist()
                self.assertIn("bin/lexicon.exe", names)
                self.assertIn("bin/arcana.exe", names)
                self.assertIn("adapters/python/adapter.py", names)
                self.assertIn("install.py", names)
                self.assertNotIn("bin/grimoire.exe", names)
                self.assertFalse(any(name.startswith("native/") for name in names))
                self.assertFalse(any(name.startswith("skills/grimoire/") for name in names))

            extracted = root / "extracted"
            with zipfile.ZipFile(combined) as archive:
                archive.extractall(extracted)
            installed = root / "installed"
            bundle_installer.install(extracted, installed, [])
            self.assertTrue((installed / "lexicon.exe").is_file())
            self.assertTrue((installed / "arcana.exe").is_file())
            self.assertTrue((installed / "adapters" / "python" / "adapter.py").is_file())
            self.assertFalse((installed / "grimoire.exe").exists())

            subset = root / "lexicon-only"
            workflow.install(build, subset, ("lexicon",))
            self.assertTrue((subset / "lexicon.exe").is_file())
            self.assertFalse((subset / "arcana.exe").exists())

    def test_build_defaults_to_surviving_components(self) -> None:
        with tempfile.TemporaryDirectory(prefix="lexicon-arcana-build-") as temporary, \
                mock.patch.object(workflow, "copy_file"), \
                mock.patch.object(workflow, "run") as run, \
                mock.patch.object(workflow, "cargo_command", return_value="cargo"), \
                mock.patch.object(workflow, "package_lexicon_adapters") as adapters, \
                mock.patch.object(workflow, "verify_arcana_protocol") as protocol, \
                mock.patch.object(workflow, "verify_versions") as versions:
            workflow.build("benchmark-test", Path(temporary) / "build")

        adapters.assert_called_once()
        protocol.assert_called_once()
        versions.assert_called_once_with(mock.ANY, "benchmark-test", ["lexicon", "arcana"])
        commands = [call.args[0] for call in run.call_args_list]
        self.assertFalse(any("./cmd/grimoire" in command for command in commands))
        self.assertFalse(any(workflow.executable_name("grimoire") in command for command in commands))

    def test_component_tests_are_cpu_bounded_by_default(self) -> None:
        calls: list[tuple[list[str], Path, dict[str, str] | None]] = []

        def record(command: list[str], cwd: Path, env: dict[str, str] | None = None) -> None:
            calls.append((list(command), cwd, env))

        with mock.patch.object(workflow, "cargo_command", return_value="cargo"), \
                mock.patch.object(workflow, "pitlord_command", return_value="pitlord"), \
                mock.patch.object(workflow, "run", side_effect=record):
            workflow.test()

        self.assertEqual(len(calls), 8)
        self.assertEqual(calls[0][0], ["pitlord", "validate", "--policy", "tools/pitlord/policy.json"])
        self.assertEqual(calls[2][0], [str(workflow.sys.executable), "scripts/check_docs.py"])
        self.assertEqual(calls[3][1], workflow.ROOT / "lexicon")
        self.assertEqual(calls[4][1], workflow.ROOT / "lexicon" / "adapters" / "java")
        self.assertEqual(calls[5][1], workflow.ROOT / "lexicon" / "adapters" / "kotlin")
        self.assertEqual(calls[6][0], [str(workflow.sys.executable), "lexicon/adapters/csharp/tests/test_adapter.py"])
        self.assertIn("arcana", " ".join(calls[7][0]))
        for _, _, environment in calls:
            self.assertEqual(environment["GOMAXPROCS"], "1")
            self.assertEqual(environment["CARGO_BUILD_JOBS"], "1")
            self.assertEqual(environment["RUST_TEST_THREADS"], "1")

    def test_arcana_protocol_verification_rejects_incomplete_capabilities(self) -> None:
        responses = [
            mock.Mock(returncode=0, stdout="", stderr=""),
            mock.Mock(
                returncode=0,
                stdout='{"ok":true,"result":{"protocol":"arcana.query.v1","version":1,"operations":["stats"]}}\n',
                stderr="",
            ),
        ]
        with tempfile.TemporaryDirectory(prefix="arcana-protocol-") as temporary:
            build = Path(temporary)
            (build / "bin").mkdir(parents=True)
            (build / "bin" / workflow.executable_name("arcana")).write_bytes(b"arcana")
            with mock.patch.object(workflow.subprocess, "run", side_effect=responses):
                with self.assertRaisesRegex(RuntimeError, "missing required operations"):
                    workflow.verify_arcana_protocol(build)

    def test_release_jobs_default_and_override(self) -> None:
        selected = workflow.parse_args(["build", "--component", "lexicon", "--component", "arcana"])
        self.assertEqual(selected.components, ["lexicon", "arcana"])
        default = workflow.parse_args(["release", "--version", "1.2.3"])
        self.assertEqual(default.jobs, 1)
        overridden = workflow.parse_args(["release", "--version", "1.2.3", "--jobs", "3"])
        self.assertEqual(overridden.jobs, 3)
        with self.assertRaises(ValueError):
            workflow.validate_jobs(0)
        with self.assertRaises(ValueError):
            workflow.resolve_install_components(("grimoire",))

    def test_version_validation_rejects_path_values(self) -> None:
        with self.assertRaises(ValueError):
            workflow.validate_version("../outside")


if __name__ == "__main__":
    run_smoke()

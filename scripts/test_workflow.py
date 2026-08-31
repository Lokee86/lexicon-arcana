#!/usr/bin/env python3
"""Deterministic smoke checks for the root release workflow."""

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
        with tempfile.TemporaryDirectory(prefix="grimoire-workflow-smoke-") as temporary:
            root = Path(temporary)
            build = root / "build"
            (build / "bin").mkdir(parents=True)
            (build / "native").mkdir()
            (build / "adapters" / "python").mkdir(parents=True)
            (build / "adapters" / "python" / "adapter.py").write_text("pass\n", encoding="utf-8")
            (build / "adapters" / "go").mkdir()
            (build / "adapters" / "go" / "lexicon-go.exe").write_bytes(b"adapter")
            (build / "skills" / "grimoire").mkdir(parents=True)
            (build / "skills" / "grimoire" / "SKILL.md").write_text(
                "---\nname: grimoire\n---\n",
                encoding="utf-8",
            )
            for name in ("grimoire.exe", "lexicon.exe", "arcana.exe"):
                (build / "bin" / name).write_bytes(name.encode())
            (build / "native" / "lodestone_ffi.dll").write_bytes(b"dll")

            release_root = workflow.package_artifacts(
                build, root / "dist", "1.2.3", platform_name="Windows", machine="AMD64"
            )
            self.assertTrue((release_root / "SHA256SUMS.txt").is_file())
            checksum_text = (release_root / "SHA256SUMS.txt").read_bytes()
            self.assertNotIn(b"\r", checksum_text)
            archives = sorted(release_root.glob("*.zip"))
            self.assertEqual(len(archives), 4)
            for line in checksum_text.decode().splitlines():
                digest, _, name = line.partition("  ")
                self.assertEqual(digest, hashlib.sha256((release_root / name).read_bytes()).hexdigest())
            combined = release_root / "grimoire-bundle-1.2.3-windows-x86_64.zip"
            with zipfile.ZipFile(combined) as archive:
                self.assertIn("bin/grimoire.exe", archive.namelist())
                self.assertIn("native/lodestone_ffi.dll", archive.namelist())
                self.assertIn("install.py", archive.namelist())
                self.assertIn("LICENSE.md", archive.namelist())
                self.assertIn("LICENSING.md", archive.namelist())
                self.assertIn("THIRD_PARTY_NOTICES.md", archive.namelist())
                self.assertIn("licenses/lodestone-Apache-2.0.txt", archive.namelist())
                self.assertIn("skills/grimoire/SKILL.md", archive.namelist())
                self.assertIn("adapters/python/adapter.py", archive.namelist())
                self.assertIn("adapters/go/lexicon-go.exe", archive.namelist())
                self.assertEqual((archive.getinfo("bin/grimoire.exe").external_attr >> 16) & 0o777, 0o755)
                self.assertEqual((archive.getinfo("adapters/go/lexicon-go.exe").external_attr >> 16) & 0o777, 0o755)
                self.assertEqual((archive.getinfo("install.py").external_attr >> 16) & 0o777, 0o755)

            extracted = root / "extracted-bundle"
            with zipfile.ZipFile(combined) as archive:
                archive.extractall(extracted)
            bundled_bin = root / "bundled-bin"
            bundled_skills = root / "bundled-skills"
            bundle_installer.install(
                extracted,
                bundled_bin,
                ["grimoire", "lexicon", "arcana"],
                [bundled_skills],
            )
            self.assertTrue((bundled_bin / "adapters" / "python" / "adapter.py").is_file())
            self.assertTrue((bundled_bin / "adapters" / "go" / "lexicon-go.exe").is_file())
            self.assertTrue((bundled_skills / "grimoire" / "SKILL.md").is_file())

            grimoire_only = root / "grimoire-component"
            bundle_installer.install(extracted, grimoire_only, ["grimoire"], [])
            self.assertTrue((grimoire_only / "grimoire.exe").is_file())
            self.assertTrue((grimoire_only / "lexicon.exe").is_file())
            self.assertTrue((grimoire_only / "arcana.exe").is_file())
            self.assertTrue((grimoire_only / "adapters" / "python" / "adapter.py").is_file())

            installed = root / "selected-bin"
            shared_skills = root / ".agents" / "skills"
            hermes_skills = root / ".hermes" / "skills"
            workflow.install(build, installed, skill_roots=(shared_skills, hermes_skills))
            self.assertEqual((installed / "grimoire.exe").read_bytes(), b"grimoire.exe")
            self.assertTrue((installed / "lodestone_ffi.dll").is_file())
            self.assertTrue((installed / "adapters" / "python" / "adapter.py").is_file())
            self.assertTrue((shared_skills / "grimoire" / "SKILL.md").is_file())
            self.assertTrue((hermes_skills / "grimoire" / "SKILL.md").is_file())

            source_grimoire_only = root / "source-grimoire-component"
            workflow.install(build, source_grimoire_only, ("grimoire",), skill_roots=())
            self.assertTrue((source_grimoire_only / "grimoire.exe").is_file())
            self.assertTrue((source_grimoire_only / "lexicon.exe").is_file())
            self.assertTrue((source_grimoire_only / "arcana.exe").is_file())
            self.assertTrue((source_grimoire_only / "adapters" / "python" / "adapter.py").is_file())

            subset = root / "lexicon-only"
            workflow.install(build, subset, ("lexicon",), skill_roots=())
            self.assertTrue((subset / "lexicon.exe").is_file())
            self.assertTrue((subset / "adapters" / "python" / "adapter.py").is_file())
            self.assertFalse((subset / "grimoire.exe").exists())
            self.assertFalse((subset / "lodestone_ffi.dll").exists())

    def test_component_tests_are_cpu_bounded_by_default(self) -> None:
        calls: list[tuple[list[str], Path, dict[str, str] | None]] = []

        def record(command: list[str], cwd: Path, env: dict[str, str] | None = None) -> None:
            calls.append((list(command), cwd, env))

        with mock.patch.object(workflow, "cargo_command", return_value="cargo"), \
                mock.patch.object(workflow, "pitlord_command", return_value="pitlord"), \
                mock.patch.object(workflow, "verify_lodestone_checkout", return_value=workflow.DEFAULT_LODESTONE_ROOT), \
                mock.patch.object(workflow, "run", side_effect=record):
            workflow.test()

        self.assertEqual(len(calls), 9)
        self.assertEqual(calls[0][0], ["pitlord", "validate", "--policy", "tools/pitlord/policy.json"])
        self.assertEqual(
            calls[1][0],
            ["pitlord", "check", "--repo", ".", "--policy", "tools/pitlord/policy.json", "--timeout", "2m"],
        )
        self.assertEqual(calls[2][0], [str(workflow.sys.executable), "scripts/check_docs.py"])
        self.assertEqual(calls[3][0], ["go", "test", "-p", "1", "-parallel", "1", "./..."])
        self.assertEqual(calls[4][0], ["go", "test", "-p", "1", "-parallel", "1", "./..."])
        self.assertEqual(calls[4][1], workflow.ROOT / "lexicon")
        self.assertEqual(calls[5][0], ["go", "test", "-p", "1", "-parallel", "1", "./..."])
        self.assertEqual(calls[5][1], workflow.ROOT / "lexicon" / "adapters" / "java")
        self.assertEqual(calls[6][0], ["go", "test", "-p", "1", "-parallel", "1", "./..."])
        self.assertEqual(calls[6][1], workflow.ROOT / "lexicon" / "adapters" / "kotlin")
        self.assertEqual(
            calls[7][0],
            [str(workflow.sys.executable), "lexicon/adapters/csharp/tests/test_adapter.py"],
        )
        self.assertEqual(calls[7][1], workflow.ROOT)
        self.assertEqual(
            calls[8][0],
            [
                "cargo", "test", "--jobs", "1", "--all-targets", "--locked",
                "--manifest-path", str(workflow.ROOT / "arcana" / "Cargo.toml"),
                "--", "--test-threads", "1",
            ],
        )
        for _, _, environment in calls:
            self.assertIsNotNone(environment)
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
        with tempfile.TemporaryDirectory(prefix="grimoire-protocol-smoke-") as temporary:
            build = Path(temporary)
            (build / "bin").mkdir(parents=True)
            (build / "bin" / workflow.executable_name("arcana")).write_bytes(b"arcana")
            with mock.patch.object(workflow.subprocess, "run", side_effect=responses):
                with self.assertRaisesRegex(RuntimeError, "missing required operations"):
                    workflow.verify_arcana_protocol(build)

    def test_arcana_protocol_verification_accepts_required_capabilities(self) -> None:
        response = {
            "ok": True,
            "result": {
                "protocol": workflow.ARCANA_PROTOCOL,
                "version": workflow.ARCANA_PROTOCOL_VERSION,
                "operations": sorted(workflow.ARCANA_REQUIRED_OPERATIONS | {"capabilities"}),
            },
        }
        responses = [
            mock.Mock(returncode=0, stdout="", stderr=""),
            mock.Mock(returncode=0, stdout=workflow.json.dumps(response) + "\n", stderr=""),
        ]
        with tempfile.TemporaryDirectory(prefix="grimoire-protocol-smoke-") as temporary:
            build = Path(temporary)
            (build / "bin").mkdir(parents=True)
            (build / "bin" / workflow.executable_name("arcana")).write_bytes(b"arcana")
            with mock.patch.object(workflow.subprocess, "run", side_effect=responses):
                workflow.verify_arcana_protocol(build)

    def test_release_jobs_default_and_override(self) -> None:
        default = workflow.parse_args(["release", "--version", "1.2.3"])
        self.assertEqual(default.jobs, 1)
        overridden = workflow.parse_args(["release", "--version", "1.2.3", "--jobs", "3"])
        self.assertEqual(overridden.jobs, 3)
        with self.assertRaises(ValueError):
            workflow.validate_jobs(0)

    def test_version_validation_rejects_path_values(self) -> None:
        with self.assertRaises(ValueError):
            workflow.validate_version("../outside")


if __name__ == "__main__":
    run_smoke()

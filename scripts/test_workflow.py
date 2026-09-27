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
            (build / "adapters" / "go-semantic").mkdir(parents=True)
            (build / "adapters" / "go-semantic" / "lexicon-go-semantic.exe").write_bytes(b"helper")
            (build / "skills" / "lexicon-arcana").mkdir(parents=True)
            (build / "skills" / "lexicon-arcana" / "SKILL.md").write_text(
                "---\nname: lexicon-arcana\n---\n", encoding="utf-8"
            )
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
                self.assertIn("adapters/go-semantic/lexicon-go-semantic.exe", names)
                self.assertNotIn("adapters/go/lexicon-go.exe", names)
                self.assertIn("install.py", names)
                self.assertIn("skills/lexicon-arcana/SKILL.md", names)
                self.assertNotIn("bin/grimoire.exe", names)
                self.assertFalse(any(name.startswith("native/") for name in names))
                self.assertFalse(any(name.startswith("skills/grimoire/") for name in names))

            extracted = root / "extracted"
            with zipfile.ZipFile(combined) as archive:
                archive.extractall(extracted)
            installed = root / "installed"
            skills = root / "skills"
            bundle_installer.install(extracted, installed, [], [skills])
            self.assertTrue((installed / "lexicon.exe").is_file())
            self.assertTrue((installed / "arcana.exe").is_file())
            self.assertTrue((installed / "adapters" / "python" / "adapter.py").is_file())
            self.assertTrue(
                (installed / "adapters" / "go-semantic" / "lexicon-go-semantic.exe").is_file()
            )
            self.assertTrue((skills / "lexicon-arcana" / "SKILL.md").is_file())
            self.assertFalse((installed / "grimoire.exe").exists())

            subset = root / "lexicon-only"
            subset_skills = root / "subset-skills"
            workflow.install(build, subset, ("lexicon",), [subset_skills])
            self.assertTrue((subset / "lexicon.exe").is_file())
            self.assertFalse((subset / "arcana.exe").exists())
            self.assertFalse((subset_skills / "lexicon-arcana" / "SKILL.md").exists())

    def test_lexicon_adapter_packaging_builds_semantic_helper_not_legacy_go_runtime(self) -> None:
        def fake_copytree(_source: Path, destination: Path, **_kwargs: object) -> None:
            (destination / "go").mkdir(parents=True)
            (destination / "go" / "oracle.go").write_text("package main\n", encoding="utf-8")
            (destination / "go-semantic").mkdir(parents=True)
            (destination / "go-semantic" / "source.go").write_text("package main\n", encoding="utf-8")

        with tempfile.TemporaryDirectory(prefix="lexicon-adapter-cutover-") as temporary, \
                mock.patch.object(workflow.shutil, "copytree", side_effect=fake_copytree), \
                mock.patch.object(workflow, "run") as run, \
                mock.patch.object(workflow, "verify_go_semantic_helper") as verify_helper, \
                mock.patch.object(workflow, "build_java_adapter"), \
                mock.patch.object(workflow, "build_csharp"), \
                mock.patch.object(workflow, "copy_file") as copy_file:
            output = Path(temporary) / "build"
            workflow.package_lexicon_adapters(
                output,
                "cargo",
                2,
                {},
            )

            adapter_root = workflow.ROOT / "lexicon" / "adapters"
            working_directories = [call.args[1] for call in run.call_args_list]
            self.assertIn(adapter_root / "go-semantic", working_directories)
            self.assertNotIn(adapter_root / "go", working_directories)
            self.assertIn(adapter_root / "c-family", working_directories)
            self.assertIn(adapter_root / "gdscript", working_directories)
            self.assertIn(adapter_root / "kotlin", working_directories)
            self.assertIn(adapter_root / "generic", working_directories)
            self.assertFalse((output / "adapters" / "go").exists())
            self.assertFalse((output / "adapters" / "go-semantic" / "source.go").exists())
            self.assertTrue((output / "adapters" / "go-semantic").is_dir())
            copy_file.assert_any_call(
                adapter_root / "go-semantic" / "VERSION",
                output / "adapters" / "go-semantic" / "VERSION",
            )
            verify_helper.assert_called_once_with(
                output / "adapters" / "go-semantic" / workflow.executable_name("lexicon-go-semantic")
            )

    def test_go_semantic_helper_version_verifier(self) -> None:
        helper = Path("lexicon-go-semantic.exe")
        expected = f"lexicon-go-semantic {workflow.go_semantic_helper_version()}"
        with mock.patch.object(
            workflow.subprocess,
            "run",
            return_value=mock.Mock(returncode=0, stdout=expected + "\n", stderr=""),
        ) as run:
            workflow.verify_go_semantic_helper(helper)
            run.assert_called_once_with(
                [helper, "--version"],
                cwd=helper.parent,
                check=True,
                capture_output=True,
                text=True,
            )

        with mock.patch.object(
            workflow.subprocess,
            "run",
            return_value=mock.Mock(returncode=0, stdout="lexicon-go-semantic stale\n", stderr=""),
        ):
            with self.assertRaisesRegex(RuntimeError, "reported .* expected"):
                workflow.verify_go_semantic_helper(helper)

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
        joined = [" ".join(str(part) for part in command) for command in commands]
        self.assertTrue(any("lexicon-cli" in command and "cargo" in command for command in joined))
        self.assertFalse(any("./cmd/lexicon" in command for command in commands))
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

        self.assertEqual(len(calls), 10)
        self.assertEqual(calls[0][0], ["pitlord", "validate", "--policy", "tools/pitlord/policy.json"])
        self.assertEqual(calls[2][0], [str(workflow.sys.executable), "scripts/check_docs.py"])
        self.assertEqual(calls[3][1], workflow.ROOT / "lexicon")
        self.assertEqual(calls[4][1], workflow.ROOT / "lexicon" / "adapters" / "java")
        self.assertEqual(calls[5][1], workflow.ROOT / "lexicon" / "adapters" / "kotlin")
        self.assertEqual(calls[6][0], [str(workflow.sys.executable), "lexicon/adapters/csharp/tests/test_adapter.py"])
        self.assertIn("lexicon/Cargo.toml", " ".join(calls[7][0]).replace("\\", "/"))
        self.assertIn("lexicon-cli/Cargo.toml", " ".join(calls[8][0]).replace("\\", "/"))
        self.assertIn("arcana/Cargo.toml", " ".join(calls[9][0]).replace("\\", "/"))
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
        skipped = workflow.parse_args([
            "install", "--bin-dir", "bin", "--skip-skills"
        ])
        self.assertTrue(skipped.skip_skills)

    def test_version_validation_rejects_path_values(self) -> None:
        with self.assertRaises(ValueError):
            workflow.validate_version("../outside")


if __name__ == "__main__":
    run_smoke()

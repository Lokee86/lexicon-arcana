from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
from typing import Any

import yaml

from benchmark_collection import collect_runs
from benchmark_component_ablation import prewarm_lexicon_arcana
from benchmark_process import isolated_path, prepare_worktree, run_checked
from benchmark_provenance import capture_provenance, file_identity

COMMON_PROMPT = r'''Read-only repository investigation. Do not modify repository files, run generators, or implement the change. Normal shell, Git, and direct file inspection are allowed. Use only the optional discovery tool available in this condition, and stop using it when direct inspection is cheaper.

Produce an implementation-grade investigation grounded in the checked-out revision. Cite every material current-behavior claim as repository-relative `path:line` or `path:start-end`. Distinguish verified current behavior from proposed design and from documentation rationale.

End with exactly one line beginning `BENCHMARK_EVIDENCE_JSON:` followed by compact JSON with one array named `evidence`. Evidence without a discovery handle must contain `path`, `symbol`, `lines`, and `claim`. When Grimoire returns an opaque inspected source-range handle, submit only that exact `handle` plus `claim`; the harness derives the canonical path and range. Do not repeat or invent immutable handle metadata.
'''


class BenchmarkEnvironment:
    def __init__(self, workspace: Path, grimoire_build: Path, model: str, provider: str):
        self.workspace = workspace
        self.grimoire_repo = workspace / "grimoire"
        self.grimoire_build = grimoire_build
        self.model = model
        self.provider = provider
        self.hermes = Path(shutil.which("hermes") or "hermes")
        self.profile_root = Path.home() / "AppData" / "Local" / "hermes" / "profiles"
        self.base_profile = self.profile_root / "benchplain"
        self.cbm_binary = workspace / "cbm-bin" / "codebase-memory-mcp.exe"
        self.cbm_proxy = self.grimoire_repo / "evaluation" / "cbm_mcp_unpaged_proxy.py"
        self.grimoire_binary = grimoire_build / "bin" / "grimoire.exe"
        self.lexicon_binary = grimoire_build / "bin" / "lexicon.exe"
        self.arcana_binary = grimoire_build / "bin" / "arcana.exe"
        self.cbm_skill = self.grimoire_repo / "evaluation" / "results" / "cbm-0.9.0-SKILL.md"
        self.grimoire_skill = grimoire_build / "skills" / "grimoire" / "SKILL.md"
        self.lexicon_arcana_skill = (
            self.grimoire_repo / "evaluation" / "skills" / "lexicon-arcana" / "SKILL.md"
        )

    def rebuild(self, version: str, conditions: tuple[str, ...], jobs: int = 1) -> None:
        components: tuple[str, ...]
        if "grimoire" in conditions:
            components = ("grimoire",)
        elif "lexicon-arcana" in conditions:
            components = ("lexicon", "arcana")
        else:
            return
        command = [
            sys.executable,
            str(self.grimoire_repo / "scripts" / "workflow.py"),
            "build", "--version", version, "--output", str(self.grimoire_build),
            "--jobs", str(jobs),
        ]
        for component in components:
            command.extend(["--component", component])
        run_checked(command, cwd=self.grimoire_repo, timeout=7200)

    def provenance(self, task_suite: Path, conditions: tuple[str, ...]) -> dict[str, Any]:
        return capture_provenance(
            repository=self.grimoire_repo,
            task_suite=task_suite,
            build_root=self.grimoire_build,
            hermes=self.hermes,
            cbm_binary=self.cbm_binary,
            cbm_skill=self.cbm_skill,
            conditions=conditions,
        )

    def profile_identity(self, name: str) -> dict[str, Any]:
        profile = self.profile_root / name
        skill = None
        for candidate in ("grimoire", "codebase-memory", "lexicon-arcana"):
            path = profile / "skills" / candidate / "SKILL.md"
            if path.is_file():
                skill = file_identity(path)
                break
        return {
            "config": file_identity(profile / "config.yaml"),
            "skill_tree": skill,
        }

    def require_dependencies(self, conditions: tuple[str, ...]) -> None:
        required = [self.hermes, self.base_profile / "config.yaml", self.base_profile / ".env"]
        if "cbm" in conditions:
            required.extend([self.cbm_binary, self.cbm_proxy, self.cbm_skill])
        if "grimoire" in conditions:
            required.extend([
                self.grimoire_binary, self.lexicon_binary, self.arcana_binary, self.grimoire_skill,
            ])
        if "lexicon-arcana" in conditions:
            required.extend([
                self.lexicon_binary, self.arcana_binary, self.lexicon_arcana_skill,
                self.grimoire_build / "adapters",
            ])
        for path in required:
            if not path.exists():
                raise RuntimeError(f"required benchmark dependency missing: {path}")

    def base_config(self) -> dict[str, Any]:
        return yaml.safe_load((self.base_profile / "config.yaml").read_text(encoding="utf-8"))

    def prepare_profile(
        self,
        name: str,
        condition: str,
        checkout: Path,
        *,
        cbm_cache: Path | None,
        audit_log: Path | None,
    ) -> None:
        profile = self.profile_root / name
        shutil.rmtree(profile, ignore_errors=True)
        profile.mkdir(parents=True, exist_ok=True)
        shutil.copy2(self.base_profile / ".env", profile / ".env")
        config = self.base_config()
        config["model"] = {
            "provider": self.provider,
            "default": self.model,
            "base_url": "https://chatgpt.com/backend-api/codex",
        }
        config.setdefault("memory", {})["memory_enabled"] = False
        config.setdefault("memory", {})["user_profile_enabled"] = False
        config["mcp_servers"] = {}
        if condition == "cbm":
            if cbm_cache is None:
                raise ValueError("CBM profile requires a cache")
            config["mcp_servers"]["codebase-memory"] = {
                "command": str(self.hermes.parent / "python.exe"),
                "args": [
                    str(self.cbm_proxy), "--binary", str(self.cbm_binary),
                    "--cwd", str(checkout), "--cache-dir", str(cbm_cache),
                ],
                "connect_timeout": 120.0,
                "enabled": True,
            }
        elif condition == "grimoire":
            arguments = ["mcp", "--root", str(checkout), "--state-mode", "refresh-if-needed"]
            if audit_log is not None:
                arguments.extend(["--audit-log", str(audit_log)])
            config["mcp_servers"]["grimoire"] = {
                "command": str(self.grimoire_binary),
                "args": arguments,
                "connect_timeout": 300.0,
                "enabled": True,
            }
        (profile / "config.yaml").write_text(yaml.safe_dump(config, sort_keys=False), encoding="utf-8")
        if condition == "cbm":
            target = profile / "skills" / "codebase-memory"
            target.mkdir(parents=True, exist_ok=True)
            shutil.copy2(self.cbm_skill, target / "SKILL.md")
        elif condition == "grimoire":
            target = profile / "skills" / "grimoire"
            target.mkdir(parents=True, exist_ok=True)
            shutil.copy2(self.grimoire_skill, target / "SKILL.md")
        elif condition == "lexicon-arcana":
            target = profile / "skills" / "lexicon-arcana"
            target.mkdir(parents=True, exist_ok=True)
            shutil.copy2(self.lexicon_arcana_skill, target / "SKILL.md")

    def prewarm_cbm(self, checkout: Path, cache: Path, output: Path) -> dict[str, Any]:
        shutil.rmtree(cache, ignore_errors=True)
        cache.mkdir(parents=True, exist_ok=True)
        environment = os.environ.copy()
        environment["CBM_CACHE_DIR"] = str(cache)
        started = time.perf_counter()
        result = run_checked(
            [str(self.cbm_binary), "cli", "index_repository", "--repo-path", str(checkout)],
            env=environment,
            timeout=1800,
        )
        elapsed = time.perf_counter() - started
        (output / "cbm-index.stdout.txt").write_text(result.stdout, encoding="utf-8")
        (output / "cbm-index.stderr.txt").write_text(result.stderr, encoding="utf-8")
        payload = json.loads(result.stdout)
        project = payload.get("project") or payload.get("name")
        if not project:
            raise RuntimeError("CBM index result omitted project name")
        return {"project": str(project), "elapsed_seconds": round(elapsed, 3)}

    def prewarm_grimoire(self, checkout: Path, output: Path) -> dict[str, Any]:
        environment = os.environ.copy()
        environment["PATH"] = isolated_path(
            self.grimoire_build / "bin",
            [self.cbm_binary.parent, self.grimoire_build / "bin"],
        )
        started = time.perf_counter()
        result = run_checked([
            str(self.grimoire_binary), "status", "--root", str(checkout), "--force",
            "--lexicon-command", str(self.lexicon_binary),
            "--arcana-command", str(self.arcana_binary),
        ], env=environment, timeout=3600)
        elapsed = time.perf_counter() - started
        (output / "grimoire-prewarm.stdout.txt").write_text(result.stdout, encoding="utf-8")
        (output / "grimoire-prewarm.stderr.txt").write_text(result.stderr, encoding="utf-8")
        payload = json.loads(result.stdout)
        return {
            "elapsed_seconds": round(elapsed, 3),
            "timings": payload.get("timings") or {},
            "actions": payload.get("actions") or [],
        }

    def prewarm_lexicon_arcana(
        self,
        checkout: Path,
        output: Path,
    ) -> tuple[dict[str, Any], dict[str, str]]:
        return prewarm_lexicon_arcana(
            checkout=checkout,
            output=output,
            lexicon_binary=self.lexicon_binary,
            arcana_binary=self.arcana_binary,
            adapters=self.grimoire_build / "adapters",
            blocked_paths=[self.cbm_binary.parent, self.grimoire_build / "bin"],
        )

    def launch(
        self,
        task: dict[str, Any],
        condition: str,
        checkout: Path,
        output: Path,
        profile: str,
        cbm_project: str | None,
        condition_context: dict[str, str] | None = None,
    ) -> tuple[subprocess.Popen[str], float, object, object, Path]:
        usage = output / f"{condition}.usage.json"
        stdout_path = output / f"{condition}.stdout.txt"
        stderr_path = output / f"{condition}.stderr.txt"
        prompt = COMMON_PROMPT + "\nTask:\n" + task["prompt"] + "\n"
        if condition == "cbm" and cbm_project:
            prompt += f"\nThe isolated CBM project is `{cbm_project}`. Use no other indexed project.\n"
        command = [
            str(self.hermes), "-p", profile, "-m", self.model,
            "--provider", self.provider, "--usage-file", str(usage),
        ]
        if condition == "cbm":
            command.extend(["--skills", "codebase-memory"])
        elif condition == "grimoire":
            command.extend(["--skills", "grimoire"])
        elif condition == "lexicon-arcana":
            command.extend(["--skills", "lexicon-arcana"])
        command.extend(["-z", prompt])
        environment = os.environ.copy()
        environment["HERMES_ACCEPT_HOOKS"] = "1"
        blocked = [self.cbm_binary.parent, self.grimoire_build / "bin"]
        selected = self.cbm_binary.parent if condition == "cbm" else self.grimoire_build / "bin" if condition == "grimoire" else None
        if condition == "lexicon-arcana":
            if condition_context is None:
                raise ValueError("Lexicon/Arcana condition requires prepared component context")
            selected = Path(condition_context["bin_dir"])
            environment["LEXICON_BENCH_EXPORT"] = condition_context["export_dir"]
            environment["ARCANA_BENCH_SNAPSHOT"] = condition_context["arcana_snapshot"]
            environment["LEXICON_BENCH_REPO"] = str(checkout)
        environment["PATH"] = isolated_path(selected, blocked)
        stdout_file = stdout_path.open("w", encoding="utf-8", newline="")
        stderr_file = stderr_path.open("w", encoding="utf-8", newline="")
        process = subprocess.Popen(command, cwd=checkout, env=environment, stdout=stdout_file, stderr=stderr_file, text=True)
        return process, time.perf_counter(), stdout_file, stderr_file, usage

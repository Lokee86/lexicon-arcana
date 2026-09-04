from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
from typing import Any, Iterable

PROVENANCE_SCHEMA = "lexicon-arcana.agent-benchmark.provenance.v1"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return "sha256:" + digest.hexdigest()


def sha256_tree(root: Path) -> str:
    digest = hashlib.sha256()
    files = sorted(path for path in root.rglob("*") if path.is_file())
    for path in files:
        relative = path.relative_to(root).as_posix().encode("utf-8")
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        file_digest = sha256_file(path).encode("ascii")
        digest.update(len(file_digest).to_bytes(8, "big"))
        digest.update(file_digest)
    return "sha256:" + digest.hexdigest()


def git_commit(repository: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
        timeout=300,
    )
    return result.stdout.strip()


def git_changes(repository: Path) -> list[str]:
    result = subprocess.run(
        ["git", "status", "--porcelain", "--untracked-files=all"],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
        timeout=300,
    )
    return [line for line in result.stdout.splitlines() if line.strip()]


def command_version(path: Path, arguments: Iterable[str]) -> str:
    result = subprocess.run(
        [str(path), *arguments],
        cwd=path.parent,
        check=True,
        capture_output=True,
        text=True,
        timeout=120,
    )
    return (result.stdout or result.stderr).strip()


def file_identity(path: Path, *, version_arguments: Iterable[str] | None = None) -> dict[str, Any]:
    identity: dict[str, Any] = {
        "path": str(path.resolve()),
        "sha256": sha256_file(path),
        "bytes": path.stat().st_size,
    }
    if version_arguments is not None:
        identity["version"] = command_version(path, version_arguments)
    return identity


def capture_provenance(
    *,
    repository: Path,
    task_suite: Path,
    build_root: Path,
    hermes: Path,
    cbm_binary: Path,
    cbm_skill: Path,
    conditions: Iterable[str],
) -> dict[str, Any]:
    conditions = tuple(conditions)
    binaries = build_root / "bin"
    tools: dict[str, Any] = {
        "hermes": file_identity(hermes, version_arguments=("--version",)),
    }
    skills: dict[str, Any] = {}
    build: dict[str, Any] = {}
    if "cbm" in conditions:
        tools["cbm"] = file_identity(cbm_binary)
        skills["cbm"] = file_identity(cbm_skill)
    if "lexicon-arcana" in conditions:
        tools["lexicon"] = file_identity(binaries / "lexicon.exe", version_arguments=("version",))
        tools["arcana"] = file_identity(binaries / "arcana.exe", version_arguments=("--version",))
        build["adapters_sha256"] = sha256_tree(build_root / "adapters")
        skills["lexicon-arcana"] = file_identity(
            repository / "evaluation" / "skills" / "lexicon-arcana" / "SKILL.md"
        )
    return {
        "schema": PROVENANCE_SCHEMA,
        "harness_commit": git_commit(repository),
        "task_suite": file_identity(task_suite),
        "tools": tools,
        "skills": skills,
        "build": build,
    }


def assert_frozen(actual: dict[str, Any], expected: dict[str, Any]) -> None:
    if actual == expected:
        return
    expected_text = json.dumps(expected, sort_keys=True)
    actual_text = json.dumps(actual, sort_keys=True)
    raise RuntimeError(
        "benchmark provenance changed after preflight; refusing mixed run\n"
        f"expected: {expected_text}\nactual: {actual_text}"
    )


def verify_build_version(provenance: dict[str, Any], version: str) -> None:
    expected = {
        "lexicon": f"lexicon version {version}",
        "arcana": f"Arcana {version}",
    }
    tools = provenance.get("tools") or {}
    for name, value in expected.items():
        if name not in tools:
            continue
        actual = (tools.get(name) or {}).get("version")
        if actual != value:
            raise RuntimeError(f"{name} reported {actual!r}; expected frozen build version {value!r}")

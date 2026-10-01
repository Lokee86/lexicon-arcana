"""Pinned, disposable Git fixture and canonical fact export for Phase 5."""
from __future__ import annotations

import hashlib
import json
import platform
from pathlib import Path
import subprocess
import tarfile

from incremental_phase0_support import seed_source

PINNED_HERMES = "7798df0a83721df7ff44b9ba023d56b85b351d1e"


def file_sha(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def prepare(repo: Path, source: Path, subset: int, source_path: str | None = None) -> dict:
    if source_path:
        repo.mkdir(parents=True)
        revision = subprocess.check_output(
            ["git", "-C", str(source), "rev-parse", "HEAD"], text=True,
        ).strip()
        archive = repo.parent / "pinned-package.tar"
        with archive.open("wb") as output:
            subprocess.run(
                ["git", "-C", str(source), "archive", "--format=tar",
                 "HEAD", source_path],
                stdout=output, check=True, timeout=60,
            )
        with tarfile.open(archive) as stored:
            stored.extractall(repo, filter="data")
        archive.unlink()
    else:
        revision = seed_source(repo, source, 0)
    if revision != PINNED_HERMES:
        raise RuntimeError(f"wrong Hermes revision: {revision}; expected {PINNED_HERMES}")
    all_python = sorted(repo.rglob("*.py"))
    if not all_python:
        raise RuntimeError("pinned tree contains no Python files")
    if subset:
        selected = set(all_python[:subset])
        for path in list(selected):
            for parent in path.parents:
                if parent == repo:
                    break
                init = parent / "__init__.py"
                if init.is_file():
                    selected.add(init)
        for path in all_python:
            if path not in selected:
                path.unlink()
        python_paths = sorted(selected)
    else:
        python_paths = all_python
    hooks = repo.parent / "empty-git-hooks"
    hooks.mkdir(exist_ok=True)
    for command in [
        ["git", "init", "-q"],
        ["git", "config", "user.email", "benchmark@example.invalid"],
        ["git", "config", "user.name", "Lexicon Phase 5"],
        ["git", "config", "core.autocrlf", "false"],
        ["git", "add", "-A"],
        ["git", "-c", f"core.hooksPath={hooks}",
         "-c", "maintenance.auto=false", "-c", "gc.auto=0",
         "commit", "-qm", "pinned disposable source"],
    ]:
        subprocess.run(command, cwd=repo, check=True, capture_output=True, timeout=90)
    original = python_paths[0]
    ten = [path for path in python_paths if path != original and
           path.name != "__init__.py"][:10]
    if len(ten) != 10:
        raise RuntimeError("benchmark needs ten independent Python files")
    return {
        "pinned_revision": revision,
        "fixture": (f"pinned-hermes-path:{source_path}" if source_path else
                    "pinned-hermes-full" if not subset else
                    f"hermes-derived-python-subset-{subset}"),
        "python_count": len(python_paths),
        "original": original.relative_to(repo).as_posix(),
        "ten_files": [path.relative_to(repo).as_posix() for path in ten],
    }


def facts_digest(export_root: Path) -> dict:
    library = export_root / "python.jsonl"
    result = hashlib.sha256()
    lines = 0
    with library.open("rb") as handle:
        header = json.loads(handle.readline())
        for line in handle:
            result.update(line)
            lines += 1
    return {
        "records_sha256": result.hexdigest(),
        "fact_record_count": lines,
        "header": {key: value for key, value in header.items()
                   if key not in ("repository", "record")},
    }


def provenance(binary: Path) -> dict:
    result = {
        "machine": platform.platform(),
        "cpu_count": __import__("os").cpu_count(),
        "binary_sha256": file_sha(binary),
        "binary_bytes": binary.stat().st_size,
        "psutil_available": False,
    }
    try:
        import psutil
        result["psutil_available"] = True
        result["machine_memory_bytes"] = psutil.virtual_memory().total
    except ImportError:
        pass
    return result
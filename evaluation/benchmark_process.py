from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess


def run_checked(
    command: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    timeout: int = 1200,
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(command, cwd=cwd, env=env, text=True, capture_output=True, timeout=timeout)
    if result.returncode != 0:
        raise RuntimeError(
            f"command failed ({result.returncode}): {command}\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result


def resolve_revision(repository: Path, revision: str) -> str:
    return run_checked(["git", "rev-parse", revision], cwd=repository, timeout=300).stdout.strip()


def prepare_worktree(repository: Path, destination: Path, revision: str) -> str:
    commit = resolve_revision(repository, revision)
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists():
        try:
            run_checked(["git", "worktree", "remove", "--force", str(destination)], cwd=repository, timeout=300)
        except Exception:
            shutil.rmtree(destination, ignore_errors=True)
    run_checked(["git", "worktree", "prune"], cwd=repository, timeout=300)
    run_checked(["git", "worktree", "add", "--detach", str(destination), commit], cwd=repository, timeout=600)
    return commit


def isolated_path(selected: Path | None, blocked: list[Path]) -> str:
    denied = {str(path).casefold() for path in blocked}
    kept = [
        entry for entry in os.environ.get("PATH", "").split(os.pathsep)
        if entry and str(Path(entry)).casefold() not in denied
    ]
    if selected is not None:
        kept.insert(0, str(selected))
    return os.pathsep.join(kept)

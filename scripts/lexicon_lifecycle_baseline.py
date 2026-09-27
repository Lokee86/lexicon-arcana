#!/usr/bin/env python3
"""Capture a Lexicon-wide cold/warm lifecycle baseline without touching the source repository."""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

try:
    import psutil
except ImportError as exc:  # pragma: no cover - developer environment dependency
    raise SystemExit(
        "psutil is required for process-tree RSS sampling: python -m pip install psutil"
    ) from exc

ROOT = Path(__file__).resolve().parents[1]
LEXICON = ROOT / "lexicon"
PERF = re.compile(r"^\[lexicon-perf\] stage=([^ ]+) elapsed_ms=([0-9.]+)(.*)$")


def build_profiler() -> Path:
    subprocess.run(
        [
            "cargo",
            "build",
            "--manifest-path",
            str(LEXICON / "Cargo.toml"),
            "--release",
            "--locked",
            "--example",
            "lifecycle_profile",
        ],
        cwd=ROOT,
        check=True,
    )
    suffix = ".exe" if os.name == "nt" else ""
    return LEXICON / "target" / "release" / "examples" / f"lifecycle_profile{suffix}"


def parse_perf(stderr: str) -> list[dict[str, object]]:
    stages: list[dict[str, object]] = []
    for line in stderr.splitlines():
        match = PERF.match(line.strip())
        if not match:
            continue
        values: dict[str, object] = {
            "stage": match.group(1),
            "elapsed_ms": float(match.group(2)),
        }
        for field in match.group(3).split():
            if "=" not in field:
                continue
            key, raw = field.split("=", 1)
            try:
                values[key] = int(raw)
            except ValueError:
                try:
                    values[key] = float(raw)
                except ValueError:
                    values[key] = raw
        stages.append(values)
    return stages


def sample_tree_rss(
    process: subprocess.Popen[str],
    timeout_seconds: float | None,
) -> tuple[int, bool]:
    peak = 0
    timed_out = False
    started = time.perf_counter()
    root = psutil.Process(process.pid)
    while process.poll() is None:
        total = 0
        try:
            members = [root, *root.children(recursive=True)]
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            members = []
        seen: set[int] = set()
        for member in members:
            if member.pid in seen:
                continue
            seen.add(member.pid)
            try:
                total += member.memory_info().rss
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                pass
        peak = max(peak, total)
        if timeout_seconds is not None and time.perf_counter() - started >= timeout_seconds:
            timed_out = True
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            break
        time.sleep(0.05)
    return peak, timed_out


def run_profile(
    command: list[str],
    env: dict[str, str],
    timeout_seconds: float | None,
) -> dict[str, object]:
    with tempfile.TemporaryDirectory(prefix="lexicon-lifecycle-process-") as raw:
        stdout_path = Path(raw) / "stdout.txt"
        stderr_path = Path(raw) / "stderr.txt"
        started = time.perf_counter()
        with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open(
            "w", encoding="utf-8"
        ) as stderr:
            process = subprocess.Popen(
                command,
                cwd=ROOT,
                env=env,
                text=True,
                stdout=stdout,
                stderr=stderr,
            )
            peak_rss, timed_out = sample_tree_rss(process, timeout_seconds)
            return_code = process.wait()
        wall_ms = (time.perf_counter() - started) * 1000.0
        stdout_text = stdout_path.read_text(encoding="utf-8")
        stderr_text = stderr_path.read_text(encoding="utf-8")
    if return_code != 0 and not timed_out:
        raise RuntimeError(
            f"profile command failed ({return_code}): {' '.join(command)}\n{stderr_text}"
        )
    lines = [line for line in stdout_text.splitlines() if line.strip()]
    metadata = json.loads(lines[-1]) if lines else {}
    return {
        "completed": not timed_out,
        "timed_out": timed_out,
        "wall_ms": wall_ms,
        "peak_process_tree_rss_bytes": peak_rss,
        "metadata": metadata,
        "stages": parse_perf(stderr_text),
    }


def export_summary(
    executable: Path,
    state_root: Path,
    snapshot: str,
    language: str,
) -> dict[str, object]:
    with tempfile.TemporaryDirectory(prefix="lexicon-lifecycle-export-") as raw:
        completed = subprocess.run(
            [
                str(executable),
                "export",
                str(state_root),
                raw,
                snapshot,
                language,
            ],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=True,
        )
        return json.loads(completed.stdout.strip().splitlines()[-1])


def revision(path: Path) -> str | None:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=path,
        text=True,
        capture_output=True,
    )
    return completed.stdout.strip() if completed.returncode == 0 else None


def worktree_dirty(path: Path) -> bool | None:
    completed = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=path,
        text=True,
        capture_output=True,
    )
    if completed.returncode != 0:
        return None
    return bool(completed.stdout.strip())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("repository", type=Path)
    parser.add_argument("--language", default="python")
    parser.add_argument("--state-root", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument(
        "--scan-timeout-seconds",
        type=float,
        default=900.0,
        help="terminate and preserve a partial pathological profile after this many seconds; use 0 to disable",
    )
    args = parser.parse_args()

    repository = args.repository.resolve()
    executable = build_profiler()
    adapter_root = (LEXICON / "adapters").resolve()
    env = os.environ.copy()
    env["LEXICON_PERF"] = "1"

    temporary_state: tempfile.TemporaryDirectory[str] | None = None
    if args.state_root is None:
        temporary_state = tempfile.TemporaryDirectory(prefix="lexicon-lifecycle-state-")
        state_root = Path(temporary_state.name)
    else:
        state_root = args.state_root.resolve()
        if state_root.exists():
            shutil.rmtree(state_root)
        state_root.mkdir(parents=True)

    try:
        base = [
            str(executable),
            "scan",
            str(repository),
            str(state_root),
            str(adapter_root),
            args.language,
        ]
        timeout_seconds = (
            None if args.scan_timeout_seconds <= 0 else args.scan_timeout_seconds
        )
        cold = run_profile(base, env, timeout_seconds)
        warm: dict[str, object] | None = None
        exported: dict[str, object] | None = None
        if cold["completed"]:
            warm = run_profile(base, env, timeout_seconds)
            snapshot = str(cold["metadata"]["snapshot_id"])
            exported = export_summary(executable, state_root, snapshot, args.language)
        result = {
            "repository": str(repository),
            "repository_revision": revision(repository),
            "repository_worktree_dirty": worktree_dirty(repository),
            "lexicon_revision": revision(ROOT),
            "lexicon_worktree_dirty": worktree_dirty(ROOT),
            "build_profile": "release",
            "language": args.language,
            "scan_timeout_seconds": timeout_seconds,
            "classification": "pathological" if cold["timed_out"] else "completed",
            "cold": cold,
            "warm": warm,
            "final": exported,
        }
        rendered = json.dumps(result, indent=2, sort_keys=True)
        print(rendered)
        if args.output is not None:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(rendered + "\n", encoding="utf-8")
    finally:
        if temporary_state is not None:
            temporary_state.cleanup()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Measure one repo-built Arcana sync and record its published snapshot."""

from __future__ import annotations

import argparse
import json
import subprocess
import time
from pathlib import Path

import psutil


def sample_tree(process: subprocess.Popen[str]) -> int:
    peak = 0
    root = psutil.Process(process.pid)
    while process.poll() is None:
        try:
            members = [root, *root.children(recursive=True)]
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            members = []
        total = 0
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
        time.sleep(0.02)
    return peak


def manifest_fields(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            values[key] = value
    return values


def snapshot_summary(state: Path) -> dict[str, object]:
    current = (state / "CURRENT").read_text(encoding="utf-8").strip()
    directory = state / "snapshots" / current.removeprefix("sha256:")
    files = {
        path.name: path.stat().st_size
        for path in sorted(directory.iterdir())
        if path.is_file()
    }
    manifest = manifest_fields(directory / "repository.manifest")
    return {
        "current": current,
        "directory": str(directory),
        "files": files,
        "total_bytes": sum(files.values()),
        "manifest": manifest,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--lexicon", type=Path, required=True)
    parser.add_argument("--state", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    command = [
        str(args.binary.resolve()),
        "sync",
        "--lexicon",
        str(args.lexicon.resolve()),
        "--state",
        str(args.state.resolve()),
    ]
    started = time.perf_counter()
    process = subprocess.Popen(
        command,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    peak_rss = sample_tree(process)
    stdout, stderr = process.communicate()
    wall_s = time.perf_counter() - started
    if process.returncode != 0:
        raise SystemExit(
            f"arcana sync failed ({process.returncode})\nstdout:\n{stdout}\nstderr:\n{stderr}"
        )

    result = {
        "command": command,
        "wall_seconds": wall_s,
        "peak_process_tree_rss_bytes": peak_rss,
        "stdout": stdout,
        "stderr": stderr,
        "snapshot": snapshot_summary(args.state),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

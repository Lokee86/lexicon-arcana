#!/usr/bin/env python3
"""Freeze Phase-2 C-family semantic and performance baselines on pinned corpora."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import tempfile
import time
from pathlib import Path

from c_family_phase2_metrics import summarize_facts

try:
    import psutil
except ImportError as exc:
    raise SystemExit("psutil is required for process-tree RSS sampling") from exc

ROOT = Path(__file__).resolve().parents[1]
LEXICON = ROOT / "lexicon"
PERF = re.compile(r"^\[lexicon-perf\] stage=([^ ]+) elapsed_ms=([0-9.]+)(.*)$")

CASES = (
    ("git", Path("git")),
    ("codebase-memory", Path("codebase-memory-mcp")),
    ("leveldb", Path("leveldb")),
    ("fmt", Path("fmt")),
    ("catch2", Path("catch2/src")),
    ("nlohmann-json", Path("nlohmann-json/include/nlohmann")),
)


def executable_name(name: str) -> str:
    return name + ".exe" if os.name == "nt" else name


def build_adapter_eval() -> Path:
    subprocess.run(
        [
            "cargo",
            "build",
            "--manifest-path",
            str(LEXICON / "Cargo.toml"),
            "--release",
            "--locked",
            "--example",
            "adapter_eval",
        ],
        cwd=ROOT,
        check=True,
    )
    return LEXICON / "target" / "release" / "examples" / executable_name("adapter_eval")


def revision(path: Path) -> str | None:
    completed = subprocess.run(
        ["git", "-C", str(path), "rev-parse", "HEAD"],
        text=True,
        capture_output=True,
    )
    return completed.stdout.strip() if completed.returncode == 0 else None


def terminate_tree(process: subprocess.Popen[str], root: psutil.Process) -> None:
    try:
        members = [*root.children(recursive=True), root]
    except (psutil.NoSuchProcess, psutil.AccessDenied):
        members = []

    for member in reversed(members):
        try:
            member.terminate()
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            pass

    _, alive = psutil.wait_procs(members, timeout=5)
    for member in alive:
        try:
            member.kill()
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            pass
    if alive:
        psutil.wait_procs(alive, timeout=5)

    try:
        process.wait(timeout=1)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()


def sample_tree_rss(process: subprocess.Popen[str], timeout: float) -> tuple[int, bool]:
    root = psutil.Process(process.pid)
    peak = 0
    started = time.perf_counter()
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
        if time.perf_counter() - started >= timeout:
            terminate_tree(process, root)
            return peak, True
        time.sleep(0.02)
    return peak, False


def parse_perf(stderr: str) -> dict[str, dict[str, float | int | str]]:
    stages: dict[str, dict[str, float | int | str]] = {}
    for line in stderr.splitlines():
        match = PERF.match(line)
        if not match:
            continue
        values: dict[str, float | int | str] = {"elapsed_ms": float(match.group(2))}
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

        stage = match.group(1)
        existing = stages.get(stage)
        if existing is None:
            stages[stage] = values
            continue

        for key, value in values.items():
            previous = existing.get(key)
            if isinstance(previous, (int, float)) and isinstance(value, (int, float)):
                if key == "jobs":
                    existing[key] = max(previous, value)
                else:
                    existing[key] = previous + value
            elif previous is None:
                existing[key] = value
            elif previous != value:
                existing[key] = value
    return stages


def current_adapter_version() -> str:
    source = (LEXICON / "src" / "adapters" / "c_family" / "mod.rs").read_text(
        encoding="utf-8"
    )
    match = re.search(r'const ADAPTER_VERSION: &str = "([^"]+)";', source)
    if not match:
        raise RuntimeError("unable to locate C-family adapter version")
    return match.group(1)


def run_once(executable: Path, repository: Path, facts: Path, timeout: float) -> dict:
    started = time.perf_counter()
    process = subprocess.Popen(
        [str(executable), "c-family", str(repository), str(facts)],
        cwd=ROOT,
        text=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        env={**os.environ, "LEXICON_PERF": "1"},
    )
    peak_rss, timed_out = sample_tree_rss(process, timeout)
    return_code = process.wait()
    stderr = process.stderr.read() if process.stderr is not None else ""
    wall_ms = (time.perf_counter() - started) * 1000.0
    if return_code != 0 and not timed_out:
        raise RuntimeError(f"adapter_eval failed for {repository}: {stderr}")
    return {
        "completed": not timed_out,
        "wall_ms": wall_ms,
        "peak_process_tree_rss_bytes": peak_rss,
        "performance_stages": parse_perf(stderr),
    }



def run_case(executable: Path, repository: Path, timeout: float) -> dict:
    with tempfile.TemporaryDirectory(prefix="c-family-phase2-") as raw:
        temporary = Path(raw)
        cold_facts = temporary / "cold.jsonl"
        warm_facts = temporary / "warm.jsonl"

        cold = run_once(executable, repository, cold_facts, timeout)
        if not cold["completed"]:
            return {"cold": cold, "warm": None, "facts": None}

        facts_summary = summarize_facts(repository, cold_facts)
        warm = run_once(executable, repository, warm_facts, timeout)
        if warm["completed"]:
            warm_hash = hashlib.sha256(warm_facts.read_bytes()).hexdigest()
            if warm_hash != facts_summary["sha256"]:
                raise RuntimeError(
                    f"non-deterministic C-family facts for {repository}: "
                    f"{facts_summary['sha256']} != {warm_hash}"
                )
        return {"cold": cold, "warm": warm, "facts": facts_summary}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus-root", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--executable", type=Path)
    parser.add_argument("--timeout-seconds", type=float, default=600.0)
    parser.add_argument("--case", action="append", choices=[name for name, _ in CASES])
    parser.add_argument("--append", action="store_true")
    args = parser.parse_args()

    executable = args.executable.resolve() if args.executable else build_adapter_eval()
    corpus_root = args.corpus_root.resolve()
    current_revision = revision(ROOT)
    adapter_version = current_adapter_version()
    if args.append and args.output.is_file():
        result = json.loads(args.output.read_text(encoding="utf-8"))
        if result.get("schema") != "lexicon.c-family.phase2-baseline.v1":
            raise ValueError("cannot append to a different calibration schema")
        if result.get("adapter_version") != adapter_version:
            raise ValueError("cannot append calibration results from another adapter version")
    else:
        result = {
            "schema": "lexicon.c-family.phase2-baseline.v1",
            "lexicon_revision": current_revision,
            "adapter_version": adapter_version,
            "cases": {},
        }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    selected = set(args.case or [])
    for name, relative in CASES:
        if selected and name not in selected:
            continue
        repository = corpus_root / relative
        if not repository.is_dir():
            raise FileNotFoundError(repository)
        print(f"[c-family-phase2] {name}: {repository}", flush=True)
        result["cases"][name] = {
            "repository": str(repository),
            "repository_revision": revision(repository),
            **run_case(executable, repository, args.timeout_seconds),
        }
        args.output.write_text(
            json.dumps(result, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    rendered = json.dumps(result, indent=2, sort_keys=True)
    print(rendered)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
#!/usr/bin/env python3
"""Freeze Phase-2 C-family semantic and performance baselines on pinned corpora."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import tempfile
import time
from collections import Counter, defaultdict
from pathlib import Path

try:
    import psutil
except ImportError as exc:
    raise SystemExit("psutil is required for process-tree RSS sampling") from exc

ROOT = Path(__file__).resolve().parents[1]
LEXICON = ROOT / "lexicon"
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
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            return peak, True
        time.sleep(0.02)
    return peak, False


def run_once(executable: Path, repository: Path, facts: Path, timeout: float) -> dict:
    started = time.perf_counter()
    process = subprocess.Popen(
        [str(executable), "c-family", str(repository), str(facts)],
        cwd=ROOT,
        text=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
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
    }


def span_key(record: dict) -> tuple:
    span = record.get("span")
    if not isinstance(span, dict):
        return ()
    return (
        span.get("path"),
        span.get("start_line"),
        span.get("start_column"),
        span.get("end_line"),
        span.get("end_column"),
    )


def percentile90(values: list[int]) -> int:
    if not values:
        return 0
    ordered = sorted(values)
    return ordered[(len(ordered) * 9 - 1) // 10]


def summarize_facts(repository: Path, facts: Path) -> dict:
    raw = facts.read_bytes()
    counts = Counter()
    relations = Counter()
    unresolved_reasons = Counter()
    file_paths: set[str] = set()
    call_states: dict[tuple, set[str]] = defaultdict(set)
    possible_targets: dict[tuple, set[str]] = defaultdict(set)
    macro_calls = 0
    macro_references = 0
    max_macro_depth = 0

    for line in raw.splitlines():
        if not line:
            continue
        record = json.loads(line)
        kind = record.get("record")
        if kind == "node":
            counts["nodes"] += 1
            if record.get("kind") == "file":
                file_paths.add(record["path"])
        elif kind == "edge":
            counts["edges"] += 1
            relation = record["relation"]
            relations[relation] += 1
            if relation in {"calls", "possible-calls"}:
                key = (record["source"], span_key(record))
                call_states[key].add(relation)
                if relation == "possible-calls":
                    possible_targets[key].add(record["target"])
            attributes = record.get("attributes") or {}
            evidence = attributes.get("evidence") or []
            if relation in {"calls", "possible-calls"} and "macro-mediation" in evidence:
                macro_calls += 1
            if relation == "references" and attributes.get("role") == "macro-expansion":
                macro_references += 1
            depth = attributes.get("expansion_depth")
            if isinstance(depth, int):
                max_macro_depth = max(max_macro_depth, depth)
        elif kind == "unresolved":
            counts["unresolved"] += 1
            unresolved_reasons[record["reason"]] += 1
            relations[f"unresolved:{record['relation']}"] += 1
            if record.get("relation") == "calls":
                call_states[(record["source"], span_key(record))].add("unresolved")

    call_site_counts = Counter()
    for states in call_states.values():
        if "calls" in states and "possible-calls" in states:
            call_site_counts["definite_plus_possible"] += 1
        elif "calls" in states:
            call_site_counts["definite_only"] += 1
        elif "possible-calls" in states:
            call_site_counts["possible_only"] += 1
        else:
            call_site_counts["unresolved_only"] += 1

    source_bytes = 0
    for relative in file_paths:
        path = repository / relative.replace("/", os.sep)
        if path.is_file():
            source_bytes += path.stat().st_size

    fanouts = [len(targets) for targets in possible_targets.values()]
    return {
        "sha256": hashlib.sha256(raw).hexdigest(),
        "jsonl_bytes": len(raw),
        "source_files": len(file_paths),
        "source_bytes": source_bytes,
        "fact_count": sum(counts.values()),
        **dict(counts),
        "relations": dict(sorted(relations.items())),
        "unresolved_reasons": dict(sorted(unresolved_reasons.items())),
        "call_sites": {
            "total": len(call_states),
            **dict(sorted(call_site_counts.items())),
            "possible_target_fanout_p90": percentile90(fanouts),
        },
        "macro": {
            "mediated_call_edges": macro_calls,
            "expansion_reference_edges": macro_references,
            "max_expansion_depth": max_macro_depth,
        },
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
    args = parser.parse_args()

    executable = args.executable.resolve() if args.executable else build_adapter_eval()
    corpus_root = args.corpus_root.resolve()
    result = {
        "schema": "lexicon.c-family.phase2-baseline.v1",
        "lexicon_revision": revision(ROOT),
        "adapter_version": "0.5.0",
        "cases": {},
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    for name, relative in CASES:
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

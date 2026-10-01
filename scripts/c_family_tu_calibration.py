#!/usr/bin/env python3
"""Run correctness-first C-family TU calibration against pinned corpora."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

import c_family_phase2_baseline as baseline

ROOT = Path(__file__).resolve().parents[1]
HARD_CUT = ROOT / ".hardcut-build"
DEFAULT_CORPUS_SOURCE = Path(os.environ.get("LEXICON_C_FAMILY_CORPUS", "/corpus"))
CASE_ORDER = (
    "leveldb",
    "nlohmann-json",
    "fmt",
    "catch2",
    "codebase-memory",
)
CASES = {
    "leveldb": Path("leveldb"),
    "nlohmann-json": Path("nlohmann-json/include/nlohmann"),
    "fmt": Path("fmt"),
    "catch2": Path("catch2/src"),
    "codebase-memory": Path("codebase-memory-mcp"),
    "git": Path("git"),
}
CBM_MAX_COLD_MS = 63_000
CBM_MAX_COLD_RSS_BYTES = 2_812_000_000
HELPER_ENVIRONMENT = "LEXICON_C_FAMILY_CLANG_HELPER"
WORKERS_ENVIRONMENT = "LEXICON_MAX_WORKERS"


def checked_run(
    command: list[str], *, cwd: Path | None = None, env: dict[str, str] | None = None
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode:
        raise RuntimeError(
            f"command failed ({completed.returncode}): {subprocess.list2cmdline(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return completed


def run_multilang_gate(results_dir: Path, provenance: dict[str, str]) -> None:
    temp_root = HARD_CUT / "tmp"
    temp_root.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env.update({"TEMP": str(temp_root), "TMP": str(temp_root), "TMPDIR": str(temp_root)})
    completed = checked_run(
        [sys.executable, str(ROOT / "scripts" / "lexicon_perf_regression.py"), "--tier", "multilang"],
        cwd=ROOT,
        env=env,
    )
    results_dir.mkdir(parents=True, exist_ok=True)
    (results_dir / "multilang-regression.json").write_text(
        json.dumps(
            {
                "schema": "lexicon.c-family.multilang-gate.v1",
                "completed": True,
                "provenance": provenance,
                "stdout": completed.stdout,
                "stderr": completed.stderr,
            },
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n",
        encoding="utf-8",
    )


def require_preceding_cases(
    case: str, results_dir: Path, provenance: dict[str, str]
) -> None:
    multilang = results_dir / "multilang-regression.json"
    if not multilang.is_file():
        raise RuntimeError("run the multilang regression gate before C-family corpus calibration")
    multilang_record = json.loads(multilang.read_text(encoding="utf-8"))
    if not multilang_record.get("completed") or multilang_record.get("provenance") != provenance:
        raise RuntimeError("multilang regression evidence is incomplete or stale for these binaries")
    if case == "git":
        required = CASE_ORDER
    else:
        required = CASE_ORDER[: CASE_ORDER.index(case)]
    for previous_case in required:
        path = results_dir / f"{previous_case}.json"
        if not path.is_file():
            raise RuntimeError(f"calibration order requires {previous_case} before {case}")
        record = json.loads(path.read_text(encoding="utf-8"))
        result = record.get("result", {})
        cold = result.get("cold") or {}
        warm = result.get("warm") or {}
        if record.get("schema") != "lexicon.c-family.tu-calibration.v2":
            raise RuntimeError(f"{previous_case} result uses an incompatible calibration schema")
        if not result.get("gate_passed") or record.get("provenance") != provenance:
            raise RuntimeError(f"{previous_case} calibration failed or uses stale binary provenance")
        if not cold.get("completed") or not warm.get("completed"):
            raise RuntimeError(f"calibration order requires completed cold and warm {previous_case} runs")
        require_same_fact_hash(
            cold.get("canonical_fact_sha256"),
            warm.get("canonical_fact_sha256"),
            f"cold/warm {previous_case}",
        )
        if result.get("concurrency_required"):
            concurrency = result.get("concurrency") or {}
            if set(concurrency) != {"1", "2", "4"}:
                raise RuntimeError(f"{previous_case} is missing a required worker-count result")
            for count in ("1", "2", "4"):
                run = concurrency[count]
                if not run.get("completed"):
                    raise RuntimeError(f"{previous_case} worker-count {count} run did not complete")
                require_same_fact_hash(
                    cold["canonical_fact_sha256"],
                    run.get("canonical_fact_sha256"),
                    f"worker-count {count} {previous_case}",
                )


def prepare_corpus(source: Path, destination: Path) -> Path:
    source = source.resolve()
    if not source.is_dir():
        raise FileNotFoundError(f"frozen corpus source does not exist: {source}")
    if not destination.exists():
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(source, destination)
    if not destination.is_dir():
        raise NotADirectoryError(destination)
    return destination.resolve()


def prepare_compilation_database(case: str, corpus_root: Path) -> Path:
    source_root = corpus_root / CASES[case]
    build_root = HARD_CUT / "build" / case
    build_root.mkdir(parents=True, exist_ok=True)

    if case == "leveldb":
        checked_run(
            [
                "cmake", "-S", str(source_root), "-B", str(build_root),
                "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON",
                "-DLEVELDB_BUILD_TESTS=OFF", "-DLEVELDB_BUILD_BENCHMARKS=OFF",
            ],
            cwd=ROOT,
        )
        shutil.copy2(build_root / "compile_commands.json", source_root / "compile_commands.json")
    elif case == "fmt":
        checked_run(
            [
                "cmake", "-S", str(source_root), "-B", str(build_root),
                "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON", "-DFMT_TEST=OFF", "-DFMT_DOC=OFF",
            ],
            cwd=ROOT,
        )
        shutil.copy2(build_root / "compile_commands.json", source_root / "compile_commands.json")
    elif case == "catch2":
        catch_root = corpus_root / "catch2"
        checked_run(
            [
                "cmake", "-S", str(catch_root), "-B", str(build_root),
                "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON", "-DCATCH_BUILD_TESTING=OFF",
            ],
            cwd=ROOT,
        )
        shutil.copy2(build_root / "compile_commands.json", catch_root / "src" / "compile_commands.json")
    elif case == "codebase-memory":
        checked_run(
            [
                sys.executable,
                str(ROOT / "scripts" / "c_family_make_compdb.py"),
                "--repository", str(source_root), "--makefile", "Makefile.cbm",
                "--target", "cbm",
                "--fallback-target", "build/c/test-runner",
                "--fallback-target", "build/c/test-repro-runner",
                "--make-arg=CC=clang", "--make-arg=CXX=clang++",
                "--output", str(source_root / "compile_commands.json"),
            ],
            cwd=ROOT,
        )
    elif case == "git":
        checked_run(
            [
                "cmake", "-S", str(source_root / "contrib" / "buildsystems"),
                "-B", str(build_root), "-DCMAKE_BUILD_TYPE=Release",
                "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON", "-DBUILD_TESTING=OFF",
            ],
            cwd=ROOT,
        )
        shutil.copy2(build_root / "compile_commands.json", source_root / "compile_commands.json")
    # nlohmann-json intentionally uses deterministic synthetic source commands.
    return source_root


def require_codebase_memory_gate(results_dir: Path) -> dict[str, Any]:
    prior_path = results_dir / "codebase-memory.json"
    if not prior_path.is_file():
        raise RuntimeError(
            "Git calibration is outside this sequence; the required Codebase Memory "
            f"result is missing: {prior_path}"
        )
    previous = json.loads(prior_path.read_text(encoding="utf-8"))
    result = previous.get("result", {})
    cold = result.get("cold") or {}
    warm = result.get("warm") or {}
    if not cold.get("completed") or not warm.get("completed"):
        raise RuntimeError("Git calibration requires completed cold and warm Codebase Memory runs")
    if cold.get("wall_ms", float("inf")) > CBM_MAX_COLD_MS:
        raise RuntimeError(
            f"Git calibration is gated: Codebase Memory cold wall was {cold.get('wall_ms')} ms "
            f"(limit {CBM_MAX_COLD_MS} ms)"
        )
    if cold.get("peak_process_tree_rss_bytes", float("inf")) > CBM_MAX_COLD_RSS_BYTES:
        raise RuntimeError(
            f"Git calibration is gated: Codebase Memory cold RSS was "
            f"{cold.get('peak_process_tree_rss_bytes')} bytes "
            f"(limit {CBM_MAX_COLD_RSS_BYTES} bytes)"
        )
    for label, run in (("cold", cold), ("warm", warm)):
        metrics = run.get("architecture_metrics") or {}
        discovered = metrics.get("discovered_owned_files")
        claimed = metrics.get("claimed_owned_files")
        observed = metrics.get("transport_file_frames")
        if (
            discovered is None
            or claimed is None
            or observed is None
            or discovered != claimed
            or claimed != observed
        ):
            raise RuntimeError(
                f"Git calibration is gated: Codebase Memory {label} run must claim every "
                "discovered owned file and emit exactly one file frame per claim "
                f"(discovered={discovered}, claimed={claimed}, observed={observed})"
            )
    return {
        "case": "codebase-memory",
        "cold_wall_ms": cold["wall_ms"],
        "cold_peak_process_tree_rss_bytes": cold["peak_process_tree_rss_bytes"],
    }


def architecture_metrics(stages: dict[str, dict[str, Any]]) -> dict[str, int | float]:
    """Require native counters that only the direct-TU architecture can provide."""
    execution = stages.get("c-family.clang.execution", {})
    transport = stages.get("c-family.clang.observation_emission", {})
    frontend = stages.get("c-family.clang.frontend_work", {})
    required = {
        "discovered_owned_files": (execution, "discovered_owned_files"),
        "primary_real_parse_units": (execution, "primary_real_parse_units"),
        "primary_synthetic_parse_units": (execution, "primary_synthetic_parse_units"),
        "explicit_header_compile_units": (execution, "explicit_header_compile_units"),
        "orphan_fallback_tus": (execution, "orphan_fallback_units"),
        "active_clang_lanes": (execution, "active_clang_lanes"),
        "completed_tus": (execution, "completed_tus"),
        "claimed_owned_files": (execution, "claimed_owned_files"),
        "discarded_duplicate_file_observations": (execution, "discarded_duplicate_file_observations"),
        "completed_orphan_tus": (execution, "completed_orphan_tus"),
        "claimed_orphan_files": (execution, "claimed_orphan_files"),
        "discarded_duplicate_orphan_observations": (
            execution, "discarded_duplicate_orphan_observations"
        ),
        "framed_transport_bytes": (transport, "framed_transport_bytes"),
        "transport_file_frames": (transport, "observed_files"),
        "peak_helper_rss_bytes": (transport, "peak_helper_rss_bytes"),
        "transport_bytes": (transport, "transport_bytes"),
        "peak_rss_bytes": (transport, "peak_rss_bytes"),
        "frontend_wall_ms": (frontend, "elapsed_ms"),
    }
    missing = [f"{name} ({stage.get('stage', 'metric')}.{key})" for name, (stage, key) in required.items() if key not in stage]
    if missing:
        raise RuntimeError(
            "completed run lacks direct-TU calibration metrics: " + ", ".join(missing)
        )
    return {name: stage[key] for name, (stage, key) in required.items()}


def require_same_fact_hash(expected: str, actual: str, label: str) -> None:
    if not expected or not actual or actual != expected:
        raise RuntimeError(f"{label} canonical fact hash changed: {expected} != {actual}")


def run_once(
    executable: Path,
    repository: Path,
    facts: Path,
    timeout: float,
    workers: int,
) -> dict[str, Any]:
    old_workers = os.environ.get(WORKERS_ENVIRONMENT)
    os.environ[WORKERS_ENVIRONMENT] = str(workers)
    try:
        run = baseline.run_once(executable, repository, facts, timeout)
    finally:
        if old_workers is None:
            os.environ.pop(WORKERS_ENVIRONMENT, None)
        else:
            os.environ[WORKERS_ENVIRONMENT] = old_workers
    result: dict[str, Any] = {
        "completed": run["completed"],
        "workers_requested": workers,
        "wall_ms": run["wall_ms"],
        "peak_process_tree_rss_bytes": run["peak_process_tree_rss_bytes"],
        "performance_stages": run["performance_stages"],
    }
    if run["completed"]:
        result["architecture_metrics"] = architecture_metrics(run["performance_stages"])
        result["canonical_fact_sha256"] = baseline.summarize_facts(repository, facts)["sha256"]
    else:
        result["canonical_fact_sha256"] = None
        result["architecture_metrics"] = None
    return result


def run_calibration_case(
    executable: Path,
    repository: Path,
    timeout: float,
    workers: int,
    concurrency_check: bool,
) -> dict[str, Any]:
    HARD_CUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".tu-calibration-", dir=HARD_CUT) as raw:
        temp = Path(raw)
        cold = run_once(executable, repository, temp / "cold.jsonl", timeout, workers)
        if not cold["completed"]:
            return {
                "cold": cold, "warm": None, "concurrency": {},
                "concurrency_required": concurrency_check, "gate_passed": False,
                "failures": ["cold run timed out"],
            }
        warm = run_once(executable, repository, temp / "warm.jsonl", timeout, workers)
        failures: list[str] = []
        if warm["completed"]:
            if warm["canonical_fact_sha256"] != cold["canonical_fact_sha256"]:
                failures.append("cold/warm canonical fact hashes differ")
        else:
            failures.append("warm run timed out")

        concurrency: dict[str, Any] = {}
        if concurrency_check and not failures:
            expected = cold["canonical_fact_sha256"]
            for count in (1, 2, 4):
                result = run_once(executable, repository, temp / f"workers-{count}.jsonl", timeout, count)
                concurrency[str(count)] = result
                if not result["completed"]:
                    failures.append(f"worker-count {count} run timed out")
                    break
                if result["canonical_fact_sha256"] != expected:
                    failures.append(f"worker-count {count} canonical fact hash differs")
                    break
        return {
            "cold": cold,
            "warm": warm,
            "concurrency": concurrency,
            "concurrency_required": concurrency_check,
            "gate_passed": not failures,
            "failures": failures,
        }


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def calibration_provenance(adapter_eval: Path, clang_helper: Path) -> dict[str, str]:
    return {
        "lexicon_revision": baseline.revision(ROOT) or "unknown",
        "adapter_version": baseline.current_adapter_version(),
        "adapter_eval_sha256": sha256_file(adapter_eval),
        "clang_helper_sha256": sha256_file(clang_helper),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", choices=(*CASE_ORDER, "git"), required=True)
    parser.add_argument("--adapter-eval", type=Path, required=True)
    parser.add_argument("--clang-helper", type=Path, required=True)
    parser.add_argument("--corpus-source", type=Path, default=DEFAULT_CORPUS_SOURCE)
    parser.add_argument("--results-dir", type=Path, default=HARD_CUT / "results")
    parser.add_argument("--timeout-seconds", type=float, default=600.0)
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--check-concurrency", action="store_true")
    args = parser.parse_args()

    if args.workers < 1:
        parser.error("--workers must be at least 1")
    adapter_eval = args.adapter_eval.resolve()
    clang_helper = args.clang_helper.resolve()
    if not adapter_eval.is_file():
        raise FileNotFoundError(f"adapter_eval executable not found: {adapter_eval}")
    if not clang_helper.is_file():
        raise FileNotFoundError(f"native Clang helper not found: {clang_helper}")

    results_dir = args.results_dir.resolve()
    provenance = calibration_provenance(adapter_eval, clang_helper)
    git_gate = require_codebase_memory_gate(results_dir) if args.case == "git" else None
    if args.case == "leveldb":
        run_multilang_gate(results_dir, provenance)
    require_preceding_cases(args.case, results_dir, provenance)
    corpus_root = prepare_corpus(args.corpus_source, HARD_CUT / "corpus")
    repository = (corpus_root / CASES[args.case]).resolve()
    if args.case != "git":
        repository = prepare_compilation_database(args.case, corpus_root)

    old_helper = os.environ.get(HELPER_ENVIRONMENT)
    os.environ[HELPER_ENVIRONMENT] = str(clang_helper)
    try:
        try:
            result = run_calibration_case(
                adapter_eval,
                repository,
                args.timeout_seconds,
                args.workers,
                args.check_concurrency,
            )
        except Exception as exc:
            result = {
                "cold": None,
                "warm": None,
                "concurrency": {},
                "concurrency_required": args.check_concurrency,
                "gate_passed": False,
                "failures": [str(exc)],
            }
    finally:
        if old_helper is None:
            os.environ.pop(HELPER_ENVIRONMENT, None)
        else:
            os.environ[HELPER_ENVIRONMENT] = old_helper

    record: dict[str, Any] = {
        "schema": "lexicon.c-family.tu-calibration.v2",
        "case": args.case,
        "repository": str(repository),
        "repository_revision": baseline.revision(repository),
        "adapter_version": baseline.current_adapter_version(),
        "provenance": provenance,
        "result": result,
    }
    if git_gate is not None:
        record["git_gate"] = git_gate

    results_dir.mkdir(parents=True, exist_ok=True)
    output = results_dir / f"{args.case}.json"
    output.write_text(json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n", encoding="utf-8")
    print(json.dumps(record, sort_keys=True, separators=(",", ":")))
    return 0 if result.get("gate_passed") else 1


if __name__ == "__main__":
    raise SystemExit(main())

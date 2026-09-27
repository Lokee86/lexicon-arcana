#!/usr/bin/env python3
"""Run bounded Lexicon performance-regression gates from repository-built tools."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
LEXICON = ROOT / "lexicon"
CONFIG = json.loads((ROOT / "scripts/lexicon_perf_baselines.json").read_text())
PERF = re.compile(r"^\[lexicon-perf\] stage=([^ ]+) elapsed_ms=([0-9.]+)(.*)$")


def run(command: list[str], cwd: Path, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, cwd=cwd, env=env, text=True, capture_output=True, check=True)


def build_example(name: str) -> Path:
    run(["cargo", "build", "--manifest-path", str(LEXICON / "Cargo.toml"), "--example", name], ROOT)
    suffix = ".exe" if os.name == "nt" else ""
    return LEXICON / "target/debug/examples" / f"{name}{suffix}"


def build_helper(directory: Path) -> Path:
    suffix = ".exe" if os.name == "nt" else ""
    helper = directory / f"lexicon-go-semantic{suffix}"
    run(["go", "build", "-o", str(helper), "."], LEXICON / "adapters/go-semantic")
    return helper


def parse_perf(stderr: str) -> dict[str, dict[str, float | int]]:
    stages: dict[str, dict[str, float | int]] = {}
    for line in stderr.splitlines():
        match = PERF.match(line)
        if not match:
            continue
        values: dict[str, float | int] = {"elapsed_ms": float(match.group(2))}
        for field in match.group(3).split():
            if "=" not in field:
                continue
            key, raw = field.split("=", 1)
            try:
                values[key] = int(raw)
            except ValueError:
                values[key] = float(raw)
        stages[match.group(1)] = values
    return stages


def flatten(stages: dict[str, dict[str, float | int]]) -> dict[str, float]:
    result = {
        f"{stage}.{key}": float(value)
        for stage, values in stages.items()
        for key, value in values.items()
    }
    material = stages.get("go.fact_materialization", {})
    hits = float(material.get("identity_cache_hits", 0))
    misses = float(material.get("identity_cache_misses", 0))
    result["derived.identity_cache_hit_rate"] = hits / max(hits + misses, 1)
    packages = stages.get("go.packages_load", {})
    loaded = float(packages.get("loaded_packages", 0))
    peak = float(packages.get("peak_live_packages", 0))
    result["derived.peak_live_package_fraction"] = peak / max(loaded, 1)
    return result


def check_limits(name: str, spec: dict, result: dict) -> list[str]:
    failures: list[str] = []
    if expected := spec.get("sha256"):
        if result["sha256"] != expected:
            failures.append(f"{name}: SHA-256 changed: {result['sha256']}")
    if result["wall_ms"] > spec["max_wall_ms"]:
        failures.append(f"{name}: wall time {result['wall_ms']:.0f} ms > {spec['max_wall_ms']} ms")
    metrics = flatten(result["stages"])
    for key, ceiling in spec.get("max", {}).items():
        if key not in metrics or metrics[key] > ceiling:
            failures.append(f"{name}: {key}={metrics.get(key)!r} > {ceiling}")
    for key, floor in spec.get("min", {}).items():
        if key not in metrics or metrics[key] < floor:
            failures.append(f"{name}: {key}={metrics.get(key)!r} < {floor}")
    return failures


def snapshot(name: str, path: Path, spec: dict, executable: Path, helper: Path, temp: Path) -> dict:
    output = temp / f"{name}.jsonl"
    env = os.environ.copy()
    env.update(LEXICON_PERF="1", LEXICON_GO_SEMANTIC_HELPER=str(helper))
    shape = [str(value) for value in spec["shape"]]
    started = time.perf_counter()
    completed = run([str(executable), str(path), str(output), *shape], ROOT, env)
    wall_ms = (time.perf_counter() - started) * 1000
    digest = hashlib.sha256(output.read_bytes()).hexdigest().upper()
    return {"wall_ms": wall_ms, "sha256": digest, "stages": parse_perf(completed.stderr)}


def revision(path: Path) -> str:
    return run(["git", "rev-parse", "HEAD"], path).stdout.strip()


def micro_gate() -> tuple[dict, list[str]]:
    spec = CONFIG["micro"]
    executable = build_example("fact_stream_perf")
    completed = run([str(executable), str(spec["small_records"]), str(spec["large_records"])], ROOT)
    result = json.loads(completed.stdout)
    failures = []
    if result["large_ms"] > spec["max_large_ms"]:
        failures.append(f"micro: large sort {result['large_ms']:.1f} ms > {spec['max_large_ms']} ms")
    if result["scale_ratio"] > spec["max_scale_ratio"]:
        failures.append(f"micro: scale ratio {result['scale_ratio']:.2f} > {spec['max_scale_ratio']}")
    return result, failures


def selected_repositories(args: argparse.Namespace) -> dict[str, tuple[Path, dict]]:
    selected = {}
    for name, spec in CONFIG["repositories"].items():
        if name == "lexicon-self":
            path = ROOT
        else:
            override = getattr(args, name.replace("-", "_"))
            path = Path(override).resolve() if override else ROOT.parent / spec["sibling"]
        selected[name] = (path, spec)
    return selected


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tier", choices=("quick", "micro", "fixture", "repositories", "all"), default="quick")
    parser.add_argument("--demon-docs")
    parser.add_argument("--space-rocks")
    args = parser.parse_args()

    failures: list[str] = []
    if args.tier in {"quick", "micro", "all"}:
        result, found = micro_gate()
        failures += found
        print("micro", json.dumps(result, sort_keys=True))

    if args.tier in {"quick", "fixture", "repositories", "all"}:
        executable = build_example("go_adapter_snapshot")
        with tempfile.TemporaryDirectory(prefix="lexicon-perf-regression-") as raw:
            temp = Path(raw)
            helper = build_helper(temp)
            if args.tier in {"quick", "fixture", "all"}:
                spec = CONFIG["fixture"]
                result = snapshot("fixture", ROOT / spec["path"], spec, executable, helper, temp)
                failures += check_limits("fixture", spec, result)
                print("fixture", result["sha256"], f"{result['wall_ms']:.0f} ms")
            if args.tier in {"repositories", "all"}:
                for name, (path, spec) in selected_repositories(args).items():
                    if not path.is_dir():
                        failures.append(f"{name}: repository not found: {path}")
                        continue
                    if expected := spec.get("revision"):
                        actual = revision(path)
                        if actual != expected:
                            failures.append(f"{name}: revision {actual} != pinned {expected}")
                            continue
                    result = snapshot(name, path, spec, executable, helper, temp)
                    failures += check_limits(name, spec, result)
                    print(name, result["sha256"], f"{result['wall_ms']:.0f} ms")

    if failures:
        print("performance regression gate failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    print("Lexicon performance regression gate passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

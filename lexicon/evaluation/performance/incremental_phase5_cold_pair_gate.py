"""Fail-closed diagnostic gate for paired pinned agent/ cold scans, not release acceptance."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

PINNED = "7798df0a83721df7ff44b9ba023d56b85b351d1e"
EXPECTED_FILES = 302
EXPECTED_FACTS = 249297
INTERSTACK = ("nodes", "edges", "unresolved")
STAGES = ("scan.dependency_index_build", "interstack.contract_detection")


def one(metrics: list[dict], stage: str) -> dict | None:
    values = [entry for entry in metrics if entry.get("stage") == stage]
    return values[0] if len(values) == 1 else None


def inspect(baseline: dict, optimized: dict, profile: dict) -> list[str]:
    errors: list[str] = []
    def require(condition: bool, message: str) -> None:
        if not condition:
            errors.append(message)

    runs = (baseline, optimized, profile)
    for name, run in zip(("baseline", "optimized", "profile"), runs):
        fixture = run.get("fixture", {})
        step = run.get("steps", {}).get("init_full", {})
        require(run.get("source_revision") == PINNED, f"{name}: wrong source revision")
        require(fixture.get("fixture") == "pinned-hermes-path:agent"
                and fixture.get("python_count") == EXPECTED_FILES,
                f"{name}: wrong fixture")
        require(run.get("run_status") == "completed"
                and step.get("exit_code") == 0
                and not step.get("timed_out")
                and not step.get("rss_limit_exceeded")
                and bool(step.get("snapshot_id")),
                f"{name}: censored or unpublished cold scan")
        require(step.get("facts", {}).get("fact_record_count") == EXPECTED_FACTS,
                f"{name}: fact count changed")
        require(run.get("provenance", {}).get("psutil_available") is True
                and step.get("peak_sampled_rss_bytes", 0) > 0,
                f"{name}: missing sampled memory evidence")
    hashes = [run.get("steps", {}).get("init_full", {}).get("facts", {})
              .get("records_sha256") for run in runs]
    require(bool(hashes[0]) and hashes[0] == hashes[1] == hashes[2],
            "Python semantic facts differ")
    machines = [run.get("provenance", {}).get("machine") for run in runs]
    require(bool(machines[0]) and len(set(machines)) == 1,
            "benchmarks used different machines")
    binaries = [run.get("provenance", {}).get("binary_sha256") for run in runs]
    require(bool(binaries[0]) and binaries[0] != binaries[1] != binaries[2],
            "missing distinct baseline and optimised binary evidence")

    prior = baseline.get("steps", {}).get("init_full", {})
    after = optimized.get("steps", {}).get("init_full", {})
    prior_metrics, after_metrics = prior.get("metrics", []), after.get("metrics", [])
    for stage in STAGES:
        old, new = one(prior_metrics, stage), one(after_metrics, stage)
        require(old is not None and new is not None
                and 0 < new.get("elapsed_ms", 0) < old.get("elapsed_ms", 0),
                f"{stage}: missing or unimproved paired measurement")
    require(0 < after.get("wall_seconds", 0) < prior.get("wall_seconds", 0),
            "total cold scan did not improve in paired run")

    old_links = one(prior_metrics, "interstack.linking")
    new_links = one(after_metrics, "interstack.linking")
    require(old_links is not None and new_links is not None
            and all(old_links.get(key) == new_links.get(key) for key in INTERSTACK),
            "interstack cardinality changed (not an exact interstack fact oracle)")
    shards = [m for m in after_metrics
              if m.get("stage") == "scan.dependency_index_shards"]
    require(len(shards) == 5
            and sum(s.get("partitions", 0) for s in shards) == 318
            and all(1 < s.get("workers", 0) <= 8 for s in shards),
            "missing bounded immutable-shard publication evidence")
    detail = profile.get("steps", {}).get("init_full", {}).get("metrics", [])
    labels = ("interstack.constants", "interstack.consumers",
              "interstack.http_providers", "interstack.http_producers",
              "interstack.message_producers", "interstack.config_reads",
              "interstack.boundary_config", "interstack.process_contracts",
              "interstack.state_contracts")
    require(all(one(detail, name) is not None for name in labels),
            "missing per-detector profile")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("baseline", "optimized", "profile"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    reports = [json.loads(getattr(args, name).read_text(encoding="utf-8"))
               for name in ("baseline", "optimized", "profile")]
    errors = inspect(*reports)
    print("PINNED AGENT COLD DIAGNOSTIC:", "PASS" if not errors else "FAIL")
    for message in errors:
        print("-", message)
    print("Full pinned Hermes Phase 5 acceptance is NOT included.")
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())

"""Strict production-CLI work-count gate for bounded synthetic Git edits."""
from __future__ import annotations

import argparse
import json
from pathlib import Path


def stage(record: dict, name: str) -> dict | None:
    matches = [m for m in record["metrics"] if m.get("stage") == name]
    return matches[0] if len(matches) == 1 else None


def validate_edit(record: dict, expected_files: int) -> list[str]:
    errors: list[str] = []
    if record["exit_code"] != 0 or record["timed_out"]:
        return ["edit failed or timed out"]
    inventory = stage(record, "scan.source_inventory")
    query = stage(record, "scan.dependency_index_query")
    delta = stage(record, "scan.dependency_index_delta")
    safety = stage(record, "scan.topology_safety_check")
    for name, value in [
        ("source inventory", inventory),
        ("dependency query", query),
        ("dependency delta", delta),
        ("topology safety check", safety),
    ]:
        if value is None:
            errors.append(f"missing or repeated {name}")
    if errors:
        return errors
    assert (inventory is not None and query is not None
            and delta is not None and safety is not None)
    for field, actual, expected in [
        ("discovered files", inventory["discovered_files"], expected_files),
        ("indexed skips", inventory["indexed_skips"], expected_files - 1),
        ("byte-equal fallbacks", inventory["byte_equal_skips"], 0),
        ("copied files", inventory["copied_files"], 1),
        ("query fact-object reads", query["fact_object_reads"], 0),
        ("delta fact-object reads", delta["fact_object_reads"], 0),
        ("query roots", query["roots"], 1),
        ("delta changed files", delta["changed_files"], 1),
    ]:
        if actual != expected:
            errors.append(f"{field}: expected {expected}, got {actual}")
    if safety["fact_object_reads"] > safety["selected_files"]:
        errors.append("topology safety check decoded unrelated fact objects")
    if safety["selected_files"] > query["emit_files"]:
        errors.append("topology safety check exceeded indexed emission scope")
    if safety["full_required"]:
        errors.append("ordinary one-file edit unexpectedly requested full analysis")
    if query["loaded_partitions"] > 4:
        errors.append(f"query loaded {query['loaded_partitions']} partitions")
    if delta["loaded_partitions"] > 16:
        errors.append(f"delta loaded {delta['loaded_partitions']} partitions")
    if any(m["stage"] in ("scan.dependency_bootstrap", "scan.dependency_rebuild",
                         "scan.dependency_rebuild_progress")
           for m in record["metrics"]):
        errors.append("ordinary edit rebuilt repository-wide dependency data")
    return errors


def validate_run(run: dict) -> list[str]:
    errors = []
    first = run["steps"].get("init_full")
    unchanged = run["steps"].get("unchanged")
    if not first or not unchanged:
        return ["missing full-init or unchanged stage"]
    inventory = stage(unchanged, "scan.source_inventory")
    if inventory is None:
        return ["missing unchanged inventory"]
    count = inventory["discovered_files"]
    if inventory["indexed_skips"] != count or inventory["byte_equal_skips"]:
        errors.append("unchanged scan read source contents instead of using index")
    if first["snapshot_id"] != unchanged["snapshot_id"]:
        errors.append("unchanged scan published an unnecessary generation")
    for label in ("edit_once", "edit_twice"):
        record = run["steps"].get(label)
        if record is None:
            errors.append(f"missing {label}")
        else:
            errors.extend(f"{label}: {reason}" for reason in validate_edit(record, count))
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("runs", type=Path, nargs="+")
    args = parser.parse_args()
    counts = []
    partition_loads = []
    for path in args.runs:
        run = json.loads(path.read_text(encoding="utf-8"))
        errors = validate_run(run)
        if errors:
            for reason in errors:
                print(f"FAIL {path.name}: {reason}")
            return 1
        inv = stage(run["steps"]["unchanged"], "scan.source_inventory")
        counts.append(inv["discovered_files"])
        partition_loads.append(stage(run["steps"]["edit_once"],
                                     "scan.dependency_index_delta")["loaded_partitions"])
        print(f"PASS {path.name}: {counts[-1]} files; no unrelated content reads")
    if len(counts) > 1 and (max(partition_loads) - min(partition_loads) > 4):
        print(f"FAIL index partition load growth: {list(zip(counts, partition_loads))}")
        return 1
    print(f"PASS bounded dependency load counts: {list(zip(counts, partition_loads))}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

"""Compare true pinned-source Phase 5 runs; never pass on censored data."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

CASES = (
    "init_full", "unchanged", "edit_once", "edit_twice",
    "dirty_mirror", "edit_ten", "add_file", "remove_file", "rename_file",
)


def metric(step: dict, stage: str) -> dict | None:
    found = [item for item in step.get("metrics", [])
             if item.get("stage") == stage]
    return found[0] if len(found) == 1 else None


def completed(step: dict | None) -> bool:
    return bool(step and step["exit_code"] == 0 and
                not step["timed_out"] and
                not step.get("rss_limit_exceeded"))


def compare(before: dict, after: dict) -> dict:
    result = {"status": "inconclusive", "gates": {}, "rows": [],
              "mismatches": [], "limitations": []}
    same = before.get("source_revision") == after.get("source_revision")
    same &= before.get("fixture", {}).get("fixture") == after.get("fixture", {}).get("fixture")
    same &= before.get("provenance", {}).get("machine") == after.get("provenance", {}).get("machine")
    result["gates"]["comparable_inputs"] = "pass" if same else "fail"
    if not same:
        result["mismatches"].append("source revision, fixture or machine differ")
    old = before.get("steps", {})
    new = after.get("steps", {})
    for name in CASES:
        a, b = old.get(name), new.get(name)
        row = {"scenario": name,
               "before_seconds": a.get("wall_seconds") if a else None,
               "after_seconds": b.get("wall_seconds") if b else None,
               "before_rss": a.get("peak_sampled_rss_bytes") if a else None,
               "after_rss": b.get("peak_sampled_rss_bytes") if b else None,
               "before_completed": completed(a), "after_completed": completed(b)}
        if a and b and a.get("facts") and b.get("facts"):
            left, right = a["facts"], b["facts"]
            row["fact_parity"] = (left["records_sha256"] == right["records_sha256"]
                                  and left["fact_record_count"] == right["fact_record_count"])
            if not row["fact_parity"]:
                result["mismatches"].append(f"{name} exported semantic facts differ")
        else:
            row["fact_parity"] = None
        result["rows"].append(row)

    full = new.get("init_full")
    adapter = metric(full, "scan.adapter.python.full") if full else None
    full_seconds = adapter["elapsed_ms"] / 1000 if adapter else None
    result["full_adapter_seconds"] = full_seconds
    nochange = new.get("unchanged")
    edit = new.get("edit_once")
    result["gates"]["warm_nochange_under_5_s"] = (
        "pass" if completed(nochange) and nochange["wall_seconds"] <= 5
        else "fail" if completed(nochange) else "inconclusive"
    )
    result["gates"]["one_edit_under_10_s"] = (
        "pass" if completed(edit) and edit["wall_seconds"] <= 10
        else "fail" if completed(edit) else "inconclusive"
    )
    result["gates"]["one_edit_under_20pct_full_adapter"] = (
        "pass" if completed(edit) and full_seconds and edit["wall_seconds"] <= .2 * full_seconds
        else "fail" if completed(edit) and full_seconds else "inconclusive"
    )
    for name in ("edit_once", "edit_twice"):
        s = new.get(name)
        inv = metric(s, "scan.source_inventory") if s else None
        query = metric(s, "scan.dependency_index_query") if s else None
        delta = metric(s, "scan.dependency_index_delta") if s else None
        clean = (completed(s) and inv and query and delta and
                 inv["byte_equal_skips"] == 0 and
                 inv["copied_files"] == 1 and
                 query["fact_object_reads"] == 0 and
                 delta["fact_object_reads"] == 0 and
                 not any(m["stage"] in ("scan.dependency_bootstrap",
                                       "scan.dependency_rebuild")
                         for m in s["metrics"]))
        result["gates"][name + "_bounded_work"] = (
            "pass" if clean else "fail" if completed(s) else "inconclusive"
        )

    old_ten, new_ten = old.get("edit_ten"), new.get("edit_ten")
    if completed(old_ten) and completed(new_ten):
        result["gates"]["ten_file_before_after_bound"] = (
            "pass" if new_ten["wall_seconds"] <= old_ten["wall_seconds"]
            else "fail")
        result["ten_file_old_bound_s"] = old_ten["wall_seconds"]
    else:
        result["gates"]["ten_file_before_after_bound"] = "inconclusive"
        result["limitations"].append("10-file bound needs both completed runs")
    parity = [r for r in result["rows"] if r["fact_parity"] is not None]
    result["gates"]["semantic_parity"] = (
        "pass" if len(parity) == len(CASES) and all(r["fact_parity"] for r in parity)
        else "fail" if any(not r["fact_parity"] for r in parity) else "inconclusive"
    )
    for report, label in ((before, "before"), (after, "after")):
        recover = report.get("steps", {}).get("recover")
        unchanged = report.get("steps", {}).get("post_recovery_unchanged")
        result["gates"][label + "_cancellation_recovery"] = (
            "pass" if completed(recover) and completed(unchanged)
                     and recover["snapshot_id"] == unchanged["snapshot_id"]
            else "fail" if recover and unchanged else "inconclusive"
        )
    values = result["gates"].values()
    result["status"] = ("pass" if all(v == "pass" for v in values)
                        else "fail" if any(v == "fail" for v in values)
                        else "inconclusive")
    return result


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("before", type=Path)
    ap.add_argument("after", type=Path)
    ap.add_argument("--output", type=Path, required=True)
    args = ap.parse_args()
    result = compare(json.loads(args.before.read_text()),
                     json.loads(args.after.read_text()))
    args.output.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(result["status"], json.dumps(result["gates"]), flush=True)
    return int(result["status"] != "pass")


if __name__ == "__main__":
    raise SystemExit(main())

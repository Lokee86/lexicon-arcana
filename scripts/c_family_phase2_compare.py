#!/usr/bin/env python3
"""Compare a Phase-2 C-family calibration run with the frozen oracle baseline."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

FACT_FIELDS = ("fact_count", "nodes", "edges", "unresolved")
RELATIONS = (
    "calls", "possible-calls", "extends", "includes", "passes-to",
    "reads", "writes", "references", "unresolved:calls",
)
CALL_FIELDS = (
    "total", "definite_only", "definite_plus_possible", "possible_only",
    "unresolved_only", "possible_target_fanout_p90",
)
MACRO_FIELDS = ("mediated_call_edges", "expansion_reference_edges", "clang_expansion_call_edges", "max_expansion_depth")


def delta(before: int | float, after: int | float) -> dict[str, float | int | None]:
    change = after - before
    return {
        "baseline": before,
        "current": after,
        "delta": change,
        "ratio": (after / before) if before else None,
    }


def count_deltas(before: dict[str, Any], after: dict[str, Any]) -> dict[str, Any]:
    keys = sorted(set(before) | set(after))
    return {key: delta(int(before.get(key, 0)), int(after.get(key, 0))) for key in keys}


def facts_delta(before: dict[str, Any], after: dict[str, Any]) -> dict[str, Any]:
    return {
        "sha256_equal": before.get("sha256") == after.get("sha256"),
        "scalars": {
            field: delta(int(before.get(field, 0)), int(after.get(field, 0)))
            for field in FACT_FIELDS
        },
        "relations": {
            field: delta(
                int(before.get("relations", {}).get(field, 0)),
                int(after.get("relations", {}).get(field, 0)),
            )
            for field in RELATIONS
        },
        "unresolved_reasons": count_deltas(
            before.get("unresolved_reasons", {}), after.get("unresolved_reasons", {})
        ),
        "call_sites": {
            field: delta(
                int(before.get("call_sites", {}).get(field, 0)),
                int(after.get("call_sites", {}).get(field, 0)),
            )
            for field in CALL_FIELDS
        },
        "macro": {
            field: delta(
                int(before.get("macro", {}).get(field, 0)),
                int(after.get("macro", {}).get(field, 0)),
            )
            for field in MACRO_FIELDS
        },
    }


def run_delta(before: dict[str, Any] | None, after: dict[str, Any] | None) -> dict[str, Any] | None:
    if before is None or after is None:
        return None
    return {
        "completed": bool(after.get("completed")),
        "wall_ms": delta(float(before["wall_ms"]), float(after["wall_ms"])),
        "peak_process_tree_rss_bytes": delta(
            int(before["peak_process_tree_rss_bytes"]),
            int(after["peak_process_tree_rss_bytes"]),
        ),
        "performance_stages": after.get("performance_stages", {}),
    }


def compare(baseline: dict[str, Any], current: dict[str, Any]) -> dict[str, Any]:
    before_cases = baseline.get("cases", {})
    after_cases = current.get("cases", {})
    unexpected = sorted(set(after_cases) - set(before_cases))
    if unexpected:
        raise ValueError(f"current calibration contains unknown cases: {unexpected}")

    cases: dict[str, Any] = {}
    for name in sorted(after_cases):
        before = before_cases[name]
        after = after_cases[name]
        revision_match = before.get("repository_revision") == after.get("repository_revision")
        if not revision_match:
            raise ValueError(
                f"{name}: revision mismatch "
                f"{before.get('repository_revision')} != {after.get('repository_revision')}"
            )
        if after.get("facts") is None:
            raise ValueError(f"{name}: current calibration did not produce facts")
        cases[name] = {
            "repository_revision": after.get("repository_revision"),
            "revision_match": revision_match,
            "semantic": facts_delta(before["facts"], after["facts"]),
            "cold": run_delta(before.get("cold"), after.get("cold")),
            "warm": run_delta(before.get("warm"), after.get("warm")),
            "requires_semantic_adjudication": before["facts"].get("sha256")
            != after["facts"].get("sha256"),
        }

    return {
        "schema": "lexicon.c-family.phase2-calibration-delta.v1",
        "baseline_revision": baseline.get("lexicon_revision"),
        "current_revision": current.get("lexicon_revision"),
        "baseline_adapter_version": baseline.get("adapter_version"),
        "current_adapter_version": current.get("adapter_version"),
        "cases": cases,
    }


def ratio_text(value: float | None) -> str:
    return "n/a" if value is None else f"{value:.2f}x"


def markdown(report: dict[str, Any]) -> str:
    lines = [
        "# C-family Phase 2.7 calibration delta",
        "",
        "| Corpus | Facts | Calls | Possible calls | Unresolved calls | Cold wall | Peak RSS |",
        "|---|---:|---:|---:|---:|---:|---:|",
    ]
    for name, case in report["cases"].items():
        semantic = case["semantic"]
        relation = semantic["relations"]
        cold = case["cold"]
        lines.append(
            "| {name} | {facts:+d} | {calls:+d} | {possible:+d} | {unresolved:+d} | "
            "{wall} | {rss} |".format(
                name=name,
                facts=int(semantic["scalars"]["fact_count"]["delta"]),
                calls=int(relation["calls"]["delta"]),
                possible=int(relation["possible-calls"]["delta"]),
                unresolved=int(relation["unresolved:calls"]["delta"]),
                wall=ratio_text(cold["wall_ms"]["ratio"] if cold else None),
                rss=ratio_text(
                    cold["peak_process_tree_rss_bytes"]["ratio"] if cold else None
                ),
            )
        )
    lines.extend(
        [
            "",
            "Fact deltas are descriptive only. Every changed semantic hash requires "
            "corpus-level adjudication; Tree-sitter parity is not itself an acceptance target.",
            "",
        ]
    )
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--current", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--markdown", type=Path)
    args = parser.parse_args()

    baseline = json.loads(args.baseline.read_text(encoding="utf-8"))
    current = json.loads(args.current.read_text(encoding="utf-8"))
    report = compare(baseline, current)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if args.markdown:
        args.markdown.parent.mkdir(parents=True, exist_ok=True)
        args.markdown.write_text(markdown(report), encoding="utf-8")
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

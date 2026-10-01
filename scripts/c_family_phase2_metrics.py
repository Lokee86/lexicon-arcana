"""Fact-stream summarization for C-family Phase 2 calibration."""

from __future__ import annotations

import hashlib
import json
import os
from collections import Counter, defaultdict
from pathlib import Path


def _span_key(record: dict) -> tuple:
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


def _percentile90(values: list[int]) -> int:
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
    materialized_nodes: set[str] = set()
    graph_sources: set[str] = set()
    call_states: dict[tuple, set[str]] = defaultdict(set)
    possible_targets: dict[tuple, set[str]] = defaultdict(set)
    macro_calls = 0
    legacy_macro_references = 0
    clang_macro_calls = 0
    max_macro_depth = 0

    for line in raw.splitlines():
        if not line:
            continue
        record = json.loads(line)
        kind = record.get("record")
        if kind == "node":
            counts["nodes"] += 1
            materialized_nodes.add(record["id"])
            if record.get("kind") == "file":
                file_paths.add(record["path"])
            continue
        if kind == "unresolved":
            graph_sources.add(record["source"])
            counts["unresolved"] += 1
            unresolved_reasons[record["reason"]] += 1
            relations[f"unresolved:{record['relation']}"] += 1
            if record.get("relation") == "calls":
                call_states[(record["source"], _span_key(record))].add("unresolved")
            continue
        if kind != "edge":
            continue

        counts["edges"] += 1
        graph_sources.add(record["source"])
        relation = record["relation"]
        relations[relation] += 1
        if relation in {"calls", "possible-calls"}:
            key = (record["source"], _span_key(record))
            call_states[key].add(relation)
            if relation == "possible-calls":
                possible_targets[key].add(record["target"])

        attributes = record.get("attributes") or {}
        evidence = attributes.get("evidence") or []
        if relation in {"calls", "possible-calls"} and "macro-mediation" in evidence:
            macro_calls += 1
        if relation in {"calls", "possible-calls"} and "clang-macro-expansion" in evidence:
            clang_macro_calls += 1
        if relation == "references" and attributes.get("role") == "macro-expansion":
            legacy_macro_references += 1
        depth = attributes.get("expansion_depth")
        if isinstance(depth, int):
            max_macro_depth = max(max_macro_depth, depth)

    missing_sources = graph_sources - materialized_nodes
    if missing_sources:
        raise RuntimeError(
            "C-family graph sources lack materialized nodes: "
            + ", ".join(sorted(missing_sources)[:10])
        )

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
            "possible_target_fanout_p90": _percentile90(fanouts),
        },
        "macro": {
            "mediated_call_edges": macro_calls,
            "expansion_reference_edges": legacy_macro_references,
            "clang_expansion_call_edges": clang_macro_calls,
            "max_expansion_depth": max_macro_depth,
        },
    }

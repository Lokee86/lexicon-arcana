#!/usr/bin/env python3
"""Multi-language semantic/performance gate for Lexicon's registered Rust host adapters."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path


def multilang_gate(
    root: Path,
    executable: Path,
    helper: Path,
    clang_helper: Path,
    spec: dict,
) -> tuple[dict, list[str]]:
    env = os.environ.copy()
    env.update(
        LEXICON_GO_SEMANTIC_HELPER=str(helper),
        LEXICON_C_FAMILY_CLANG_HELPER=str(clang_helper),
    )
    completed = subprocess.run(
        [str(executable)],
        cwd=root,
        env=env,
        text=True,
        capture_output=True,
        check=True,
    )
    result = json.loads(completed.stdout)
    failures: list[str] = []

    expected_names = set(spec)
    actual_names = set(result)
    if actual_names != expected_names:
        failures.append(
            "multilang: adapter set changed: "
            f"actual={sorted(actual_names)} expected={sorted(expected_names)}"
        )

    for name, expected in spec.items():
        row = result.get(name)
        if row is None:
            continue
        if row["sha256"] != expected["sha256"]:
            failures.append(
                f"multilang/{name}: SHA-256 changed: {row['sha256']}"
            )
        if row["fact_count"] != expected["fact_count"]:
            failures.append(
                f"multilang/{name}: fact_count={row['fact_count']} "
                f"!= {expected['fact_count']}"
            )
        if row["wall_ms"] > expected["max_wall_ms"]:
            failures.append(
                f"multilang/{name}: wall time {row['wall_ms']:.1f} ms "
                f"> {expected['max_wall_ms']} ms"
            )

    return result, failures

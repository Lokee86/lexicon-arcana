#!/usr/bin/env python3
"""Pinned native execution baseline; run only on a small generated fixture."""
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import tempfile

from structural_ownership_test import perf_totals, run_with_perf

GOLDEN = pathlib.Path(__file__).with_name("structural_execution_baseline.json")
WORKERS = (1, 2, 4)


def create_fixture(root: pathlib.Path) -> dict:
    (root / "src").mkdir()
    (root / "build").mkdir()
    (root / "shared.h").write_text(
        "#pragma once\n#define SCALE(n) ((n) + 4)\n"
        "struct Shared { int field; };\n"
        "inline int shared(int x) { return SCALE(x); }\n", encoding="utf-8")
    (root / "external.h").write_text(
        "#pragma once\nnamespace dependency { inline int used(int x) { return x; } }\n",
        encoding="utf-8")
    (root / "orphan.h").write_text(
        "#pragma once\nint orphan(int);\n", encoding="utf-8")
    (root / "src" / "covered.cpp").write_text(
        "#pragma once\nint covered(void) { return 4; }\n", encoding="utf-8")
    (root / "src" / "real0.cpp").write_text(
        '#include "../shared.h"\n#include "../external.h"\n'
        '#include "covered.cpp"\n'
        "int real0() { Shared item{covered()}; "
        "return shared(dependency::used(item.field)); }\n", encoding="utf-8")
    (root / "src" / "real1.cpp").write_text(
        '#include "../shared.h"\n'
        "int real1() { return shared(2); }\n", encoding="utf-8")
    (root / "src" / "synthetic.cpp").write_text(
        '#include "../shared.h"\n'
        "int synthetic() { return shared(1); }\n", encoding="utf-8")
    entries = [
        {"directory": str(root / "build"),
         "arguments": ["clang++", "-std=c++17", "-DREAL_CONTEXT=1",
                       "-I..", "-c", "../src/" + name],
         "file": "../src/" + name}
        for name in ("real0.cpp", "real1.cpp")
    ]
    (root / "compile_commands.json").write_text(
        json.dumps(entries), encoding="utf-8")
    return {
        "protocol_version": 3,
        "operation": "structural",
        "repository_root": str(root),
        "owned_files": [
            "orphan.h", "shared.h", "src/covered.cpp", "src/real0.cpp",
            "src/real1.cpp", "src/synthetic.cpp",
        ],
        "context_files": ["external.h"],
        "workers": 1,
    }


def stable_response(response: dict, root: pathlib.Path) -> bytes:
    # Paths in commands and compiler evidence must not depend on temp names.
    encoded = json.dumps(response, sort_keys=True, separators=(",", ":"))
    encoded = encoded.replace(str(root), "<FIXTURE_ROOT>")
    return encoded.encode("utf-8")


def verify(helper: pathlib.Path, version: str, record: bool) -> None:
    with tempfile.TemporaryDirectory(prefix="lexicon-execution-baseline-") as temp:
        root = pathlib.Path(temp).resolve()
        request = create_fixture(root)
        outputs = []
        counters = []
        for workers in WORKERS:
            response, stderr = run_with_perf(
                helper, version, root, {**request, "workers": workers})
            execution = perf_totals(stderr, "c-family.clang.execution")
            for name, expected in (
                ("discovered_owned_files", len(request["owned_files"])),
                ("skipped_covered_source_units", 1),
                ("primary_synthetic_parse_units", 1),
                ("orphan_fallback_units", 1),
                ("claimed_owned_files", len(request["owned_files"])),
            ):
                if execution.get(name) != expected:
                    raise AssertionError(
                        f"workers={workers}: {name}={execution.get(name)} != {expected}; metrics={execution}")
            outputs.append(stable_response(response, root))
            counters.append({name: execution[name] for name in (
                "discovered_owned_files", "primary_real_parse_units",
                "skipped_covered_source_units", "primary_synthetic_parse_units",
                "orphan_fallback_units", "completed_tus", "claimed_owned_files")})
        if len(set(outputs)) != 1:
            raise AssertionError("1/2/4 workers changed complete native observations")
        digest = hashlib.sha256(outputs[0]).hexdigest()
        baseline = {
            "schema": "lexicon.c-family.execution-fixture.v1",
            "helper_version": version,
            "clang_version": response["clang_version"],
            "complete_native_sha256": digest,
            "canonical_counters": counters[0],
            "workers": list(WORKERS),
        }
        if any(counter != counters[0] for counter in counters[1:]):
            raise AssertionError("worker counts changed canonical execution counters")
        if record:
            GOLDEN.write_text(json.dumps(baseline, indent=2, sort_keys=True) + "\n",
                              encoding="utf-8")
        else:
            expected = json.loads(GOLDEN.read_text(encoding="utf-8"))
            if baseline != expected:
                raise AssertionError(
                    "native execution baseline drift:\n"
                    f"expected: {expected}\nactual: {baseline}")
        print(json.dumps({"baseline_match": not record, **baseline}, sort_keys=True))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("helper", type=pathlib.Path)
    parser.add_argument("version")
    parser.add_argument("--record", action="store_true",
                        help="Explicitly replace golden after reviewing semantic changes")
    args = parser.parse_args()
    verify(args.helper.resolve(), args.version, args.record)

#!/usr/bin/env python3
"""Bounded external-context PCH probe; never scans a supplied corpus."""
import argparse
import json
import pathlib
import statistics
import subprocess
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] /
                       "lexicon/adapters/c-family-clang"))
from structural_ownership_test import run_with_perf, perf_totals

def elapsed(metrics, stage):
    return sum(float(line.split("elapsed_ms=")[1].split()[0])
               for line in metrics.splitlines() if "stage=" + stage + " " in line)

def probe(helper, compiler, results, parser=None, include_dirs=(), repetitions=3):
    root = results / "pch-fixture"
    vendor = root / "vendor"
    vendor.mkdir(parents=True, exist_ok=True)
    header = vendor / "context.h"
    dependency = (
        '#include "' + str(parser) + '"\n' if parser else
        "static const int data[] = {" +
        ",".join(str(i) for i in range(20000)) + "};\n"
    )
    header.write_text("#pragma once\n" + dependency +
                      "static inline int external_probe(int n) { return n; }\n",
                      encoding="utf-8")
    (root / "owned.h").write_text("#pragma once\n#define VALUE 3\n", encoding="utf-8")
    (root / "main.c").write_text(
        '#include "vendor/context.h"\n#include "owned.h"\n'
        "int probe(int n) { n += VALUE; return external_probe(n); }\n", encoding="utf-8")
    flags = ["-std=c11", "-D_DEFAULT_SOURCE", "-O2", "-w",
             *["-I" + str(p) for p in include_dirs]]
    pch = vendor / "context.pch"
    t = time.perf_counter()
    completed = subprocess.run(
        [str(compiler), *flags, "-x", "c-header", str(header), "-o", str(pch)],
        capture_output=True, timeout=60,
    )
    if completed.returncode:
        raise RuntimeError(completed.stderr.decode(errors="replace"))
    build_seconds = time.perf_counter() - t
    request = {"protocol_version": 3, "operation": "structural",
               "repository_root": str(root),
               "owned_files": ["main.c", "owned.h"], "context_files": [],
               "workers": 1}
    expected = None
    records = {"source": [], "pch": []}
    for repetition in range(repetitions):
        for mode in records:
            arguments = ["clang", *flags]
            if mode == "pch":
                arguments += ["-include-pch", str(pch)]
            arguments += ["-c", "main.c"]
            (root / "compile_commands.json").write_text(json.dumps([{
                "directory": str(root), "file": "main.c",
                "arguments": arguments,
            }]), encoding="utf-8")
            t = time.perf_counter()
            response, metrics = run_with_perf(helper, "0.7.0", root, request)
            wall = time.perf_counter() - t
            # Compile arguments intentionally differ; semantic file payloads,
            # context evidence and diagnostics must remain identical.
            semantic = {k: response[k] for k in
                        ("files", "context_identities", "diagnostics")}
            if expected is None:
                expected = semantic
            if semantic != expected:
                raise RuntimeError("PCH changed semantic observations")
            records[mode].append({
                "wall_s": wall,
                "parse_ms": elapsed(metrics, "c-family.clang.tu.parsed"),
                "traversal_ms": elapsed(metrics, "c-family.clang.tu.visited"),
                "peak_rss_bytes": perf_totals(
                    metrics, "c-family.clang.observation_emission"
                )["peak_helper_rss_bytes"],
            })
            (results / f"pch-{mode}-{repetition}.stderr").write_text(metrics)
    summary = {
        "semantic_equal": True, "pch_build_s": build_seconds,
        "pch_bytes": pch.stat().st_size, "samples": records,
        "median": {mode: {key: statistics.median(row[key] for row in rows)
                          for key in rows[0]} for mode, rows in records.items()},
    }
    (results / "pch-result.json").write_text(json.dumps(summary, indent=2))
    print(json.dumps(summary, indent=2))

if __name__ == "__main__":
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--helper", type=pathlib.Path, required=True)
    cli.add_argument("--compiler", type=pathlib.Path, required=True)
    cli.add_argument("--results-dir", type=pathlib.Path, required=True)
    cli.add_argument("--parser", type=pathlib.Path)
    cli.add_argument("--repetitions", type=int, choices=(1, 2, 3), default=3)
    cli.add_argument("--include-dir", type=pathlib.Path, action="append", default=[])
    args = cli.parse_args()
    args.results_dir.mkdir(parents=True, exist_ok=True)
    probe(args.helper.resolve(), args.compiler.resolve(),
          args.results_dir.resolve(), args.parser, args.include_dir, args.repetitions)

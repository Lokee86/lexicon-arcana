#!/usr/bin/env python3
"""Measure only JSON/C/Python grammar shims, never the full corpus."""
import argparse
import json
import pathlib
import re
import statistics
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] /
                       "lexicon/adapters/c-family-clang"))
from structural_ownership_test import run_with_perf

LANGUAGES = ("json", "c", "python")

def measurements(stderr):
    rows = []
    for line in stderr.splitlines():
        if "stage=c-family.clang.tu." not in line:
            continue
        fields = dict(token.split("=", 1) for token in line.split() if "=" in token)
        rows.append({key: value if key == "stage" else float(value)
                     for key, value in fields.items()})
    return rows

def probe(corpus, helper, results, baseline=None, repetitions=3):
    root = results / "fixture"
    root.mkdir(parents=True, exist_ok=True)
    cbm = corpus / "internal/cbm"
    commands = []
    for index, language in enumerate(LANGUAGES):
        text = (cbm / f"grammar_{language}.c").read_text(encoding="utf-8")
        text = re.sub(r'(#include\s+)"(vendored/[^"]+)"',
                      lambda m: m[1] + '"' + str(cbm / m[2]) + '"', text)
        for owned_probe in (False, True):
            name = f"{index + 3 * owned_probe:02d}_{language}"
            name += "_probe.c" if owned_probe else ".c"
            body = "int owned_probe(int n) { n++; return n; }\n" if owned_probe else ""
            (root / name).write_text(text + body, encoding="utf-8")
            commands.append({
                "directory": str(root), "file": name,
                "arguments": ["clang", "-std=c11", "-D_DEFAULT_SOURCE", "-O2", "-w",
                              "-I" + str(cbm),
                              "-I" + str(cbm / "vendored/ts_runtime/include"),
                              "-I" + str(cbm / "vendored/ts_runtime/src"),
                              "-c", name],
            })
    (root / "compile_commands.json").write_text(json.dumps(commands))
    paths = sorted(command["file"] for command in commands)
    request = {"protocol_version": 3, "operation": "structural",
               "repository_root": str(root), "owned_files": paths,
               "context_files": [], "workers": 1}
    helpers = {"before": baseline, "after": helper} if baseline else {"after": helper}
    expected = None
    samples = {name: [] for name in helpers}
    for repetition in range(repetitions):
        for name, binary in helpers.items():
            started = time.perf_counter()
            response, stderr = run_with_perf(binary, "0.7.0", root, request)
            wall = time.perf_counter() - started
            if expected is None:
                expected = response
            if response != expected:
                raise RuntimeError("grammar observations changed")
            samples[name].append({"wall_s": wall, "boundaries": measurements(stderr)})
            (results / f"grammar-{name}-{repetition}.stderr").write_text(stderr)
    report = {"languages": LANGUAGES, "rank_paths": paths, "observations_equal": True,
              "samples": samples,
              "median_wall_s": {name: statistics.median(row["wall_s"] for row in rows)
                                for name, rows in samples.items()}}
    (results / "grammar-result.json").write_text(json.dumps(report, indent=2))
    print(json.dumps({k: v for k, v in report.items() if k != "samples"}, indent=2))

if __name__ == "__main__":
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--corpus", type=pathlib.Path, required=True)
    cli.add_argument("--helper", type=pathlib.Path, required=True)
    cli.add_argument("--baseline-helper", type=pathlib.Path)
    cli.add_argument("--results-dir", type=pathlib.Path, required=True)
    cli.add_argument("--repetitions", type=int, choices=(1, 2, 3), default=3)
    args = cli.parse_args()
    args.results_dir.mkdir(parents=True, exist_ok=True)
    probe(args.corpus.resolve(), args.helper.resolve(), args.results_dir.resolve(),
          args.baseline_helper.resolve() if args.baseline_helper else None,
          args.repetitions)

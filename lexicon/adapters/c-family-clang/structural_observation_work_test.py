#!/usr/bin/env python3
"""Conservative traversal and fixed-phase ownership regression fixtures."""
import json
import pathlib
import sys
from structural_ownership_test import (
    compile_database, perf_totals, run_with_perf, temporary_directory,
)

def verify(helper, version, baseline=None):
    with temporary_directory("lexicon-observation-work-") as temp:
        root = pathlib.Path(temp)
        (root / "external.h").write_text(
            "#pragma once\n"
            "namespace external {\n"
            '#include "part.inc"\n'
            "inline int helper(int n) { MAKE_LOCAL return n + macro_local; }\n"
            "static const int data[] = {" +
            ",".join(str(i) for i in range(20000)) + "};\n}\n",
            encoding="utf-8",
        )
        (root / "part.inc").write_text(
            "struct Nested { int field; };\n", encoding="utf-8",
        )
        (root / "owned.h").write_text(
            "#pragma once\n#define MAKE_LOCAL int macro_local = 3;\n"
            "inline int api(int n) { return n + 1; }\n",
            encoding="utf-8",
        )
        real = [f"real{i}.cpp" for i in range(4)]
        synthetic = [f"synthetic{i}.cpp" for i in range(4)]
        for name in real + synthetic:
            (root / name).write_text(
                '#include "owned.h"\n#include "external.h"\n'
                "int " + name.split(".")[0] +
                "(void) { return api(external::helper(1)); }\n",
                encoding="utf-8",
            )
        compile_database(root, real)
        request = {
            "protocol_version": 3, "operation": "structural",
            "repository_root": str(root),
            "owned_files": ["owned.h", "part.inc", *real, *synthetic],
            "context_files": ["external.h"], "workers": 1,
        }
        expected, old_metrics = (
            run_with_perf(baseline, version, root, request) if baseline
            else (None, "")
        )
        for workers in (1, 2, 4):
            response, metrics = run_with_perf(
                helper, version, root, {**request, "workers": workers},
            )
            if expected is None:
                expected = response
            assert response == expected, "observations changed across helper/worker count"
            files = {file["path"]: file for file in response["files"]}
            names = {
                path: {d["name"] for d in file["declarations"]}
                for path, file in files.items()
            }
            assert "Nested" in names["part.inc"], "owned nested include was pruned"
            assert "macro_local" in names["owned.h"], "owned macro expansion was pruned"
            assert all(files[name]["calls"] for name in synthetic), "synthetic calls were lost"
            traversal = perf_totals(metrics, "c-family.clang.traversal")
            assert traversal["pruned_declarations"] > 0, "external AST was not pruned"
            execution = perf_totals(metrics, "c-family.clang.execution")
            assert execution["peak_pending_estimated_bytes"] <= 64 * 1024 * 1024
            assert execution["active_clang_lanes"] == workers
            if baseline:
                old = perf_totals(old_metrics, "c-family.clang.execution")
                assert execution["discarded_duplicate_file_observations"] < old[
                    "discarded_duplicate_file_observations"
                ], "prior claims did not avoid constructing duplicate observations"
        print(json.dumps({
            "fixture": "owned nested include/macro + prior-phase shared header",
            "workers": [1, 2, 4], "baseline_equal": baseline is not None,
            "traversal": traversal, "execution": execution,
        }, sort_keys=True))

if __name__ == "__main__":
    verify(pathlib.Path(sys.argv[1]), sys.argv[2],
           pathlib.Path(sys.argv[3]) if len(sys.argv) > 3 else None)

#!/usr/bin/env python3
"""Opt-in lifetime metrics must preserve observations and cover each TU."""
import pathlib
import sys
from structural_ownership_test import (
    compile_database, run, run_with_perf, temporary_directory,
)

def verify(helper, version):
    with temporary_directory("lexicon-profile-") as temp:
        root = pathlib.Path(temp)
        (root / "vendor").mkdir()
        (root / "vendor/context.h").write_text(
            "#pragma once\nstatic int table[] = {" +
            ",".join(str(i) for i in range(20000)) + "};\n",
            encoding="utf-8",
        )
        files = ["a.c", "b.c"]
        for name in files:
            (root / name).write_text(
                '#include "vendor/context.h"\nint ' + name[0] +
                "(int n) { n++; return n; }\n", encoding="utf-8",
            )
        compile_database(root, files)
        request = {"protocol_version": 3, "operation": "structural",
                   "repository_root": str(root), "owned_files": files,
                   "context_files": [], "workers": 1}
        expected = run(helper, version, root, request)
        for workers in (1, 2):
            response, stderr = run_with_perf(
                helper, version, root, {**request, "workers": workers},
            )
            assert response == expected, "profiling or workers changed observations"
            records = []
            for line in stderr.splitlines():
                if "stage=c-family.clang.tu." in line:
                    records.append(dict(token.split("=", 1) for token in line.split()
                                        if "=" in token))
            phases = ("before_parse", "parsed", "visited",
                      "after_teardown", "after_submit")
            for rank in (0, 1):
                selected = [r for r in records if int(r["rank"]) == rank]
                assert [r["stage"].split(".")[-1] for r in selected] == list(phases)
                assert all(int(r["current_rss_bytes"]) >= 0 for r in selected)
                assert int(selected[1]["ast_allocated_bytes"]) > 0
                assert int(selected[2]["observation_estimated_bytes"]) > 0
                accesses = response["files"][rank]["accesses"]
                assert {a["relation"] for a in accesses} == {"reads", "writes"}
        print("profiling: observations, read/write classification and TU boundaries passed")

if __name__ == "__main__":
    verify(pathlib.Path(sys.argv[1]).resolve(), sys.argv[2])

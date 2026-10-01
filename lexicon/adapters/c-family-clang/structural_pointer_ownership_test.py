#!/usr/bin/env python3
"""Dependency pointer targets remain usable without becoming graph sources."""
import pathlib
import sys
from structural_ownership_test import compile_database, run, temporary_directory

def verify(helper, version):
    with temporary_directory("lexicon-context-pointer-") as temp:
        root = pathlib.Path(temp)
        (root / "vendor").mkdir()
        (root / "vendor/api.h").write_text(
            "typedef struct { int (*progress_callback)(int); } TSParseOptions;\n"
            "extern int (*context_pointer)(int);\n")
        (root / "owner.h").write_text(
            "typedef struct { int (*callback)(int); } OwnedOptions;\n")
        (root / "main.c").write_text(
            '#include "vendor/api.h"\n#include "owner.h"\n'
            "int target(int x) { return x; }\n"
            "int owner(TSParseOptions *p, OwnedOptions *q) {\n"
            "p->progress_callback = target; context_pointer = target;\n"
            "q->callback = target; return p->progress_callback(1); }\n")
        compile_database(root, ["main.c"])
        request = {"protocol_version": 3, "operation": "structural",
                   "repository_root": str(root), "owned_files": ["main.c", "owner.h"],
                   "context_files": [], "workers": 1}
        expected = None
        for workers in (1, 2, 4):
            result = run(helper, version, root, {**request, "workers": workers})
            if expected is None: expected = result
            assert result == expected
            files = {f["path"]: f for f in result["files"]}
            bindings = files["main.c"]["pointer_bindings"]
            assert len(bindings) == 1, bindings
            assert bindings[0]["pointer"]["qualified_name"].endswith("callback")
            accesses = files["main.c"]["accesses"]
            assert any(a["relation"] == "writes" and
                       "progress_callback" in a["target"]["qualified_name"]
                       for a in accesses)
            assert any("progress_callback" in c.get("expression", "")
                       for c in files["main.c"]["calls"])
        print("context pointer ownership and retained access/call observations passed")

if __name__ == "__main__":
    verify(pathlib.Path(sys.argv[1]).resolve(), sys.argv[2])

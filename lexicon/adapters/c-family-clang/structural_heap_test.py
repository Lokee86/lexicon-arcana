#!/usr/bin/env python3
"""Exercise reclaim after large excluded ASTs, retaining deterministic output."""
import pathlib
import sys
from structural_ownership_test import compile_database, run_with_perf, temporary_directory

def verify(helper, version):
    with temporary_directory("lexicon-heap-") as temp:
        root = pathlib.Path(temp)
        (root / "vendor").mkdir()
        (root / "vendor/table.h").write_text(
            "static int table[] = {" + "1," * 2000000 + "};\n")
        files = ["a.c", "b.c", "c.c"]
        for name in files:
            (root / name).write_text(
                '#include "vendor/table.h"\nint ' + name[0] +
                "(int n) { n++; return n; }\n")
        compile_database(root, files)
        request = {"protocol_version": 3, "operation": "structural",
                   "repository_root": str(root), "owned_files": files,
                   "context_files": [], "workers": 1}
        expected = None
        for workers in (1, 2):
            result, stderr = run_with_perf(
                helper, version, root, {**request, "workers": workers})
            if expected is None: expected = result
            assert result == expected
            supported = "allocator_stats_available=1" in stderr
            if supported and sys.platform.startswith("linux"):
                assert "stage=c-family.clang.heap_reclaim" in stderr, stderr
        print("large AST reclamation and worker determinism passed")

if __name__ == "__main__":
    verify(pathlib.Path(sys.argv[1]).resolve(), sys.argv[2])

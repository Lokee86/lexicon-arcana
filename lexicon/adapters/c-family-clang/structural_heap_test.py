#!/usr/bin/env python3
"""Exercise reclaim after large excluded ASTs, retaining deterministic output."""
import json
import os
import pathlib
import subprocess
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
                # Thresholds and try_lock may legitimately skip malloc_trim;
                # every completed TU must still reach the pre-handoff policy.
                attempts = [line for line in stderr.splitlines()
                            if "stage=c-family.clang.heap_reclaim" in line]
                assert len(attempts) == len(files), attempts
                for line in attempts:
                    assert "phase=real" in line and "rank=" in line, line
                    assert "attempted=" in line and "trimmed=" in line, line
                for rank in range(len(files)):
                    record = [i for i, line in enumerate(stderr.splitlines())
                              if "phase=real" in line and f"rank={rank}" in line]
                    trace = [stderr.splitlines()[i] for i in record]
                    teardown = next(i for i, line in enumerate(trace)
                                    if "stage=c-family.clang.tu.after_teardown" in line)
                    reclaim = next(i for i, line in enumerate(trace)
                                   if "stage=c-family.clang.heap_reclaim" in line)
                    admission = next(i for i, line in enumerate(trace)
                                     if "stage=c-family.clang.ordered_admission_wait" in line)
                    assert teardown < reclaim < admission, trace
        # A rejected driver invocation must terminate and reach reclamation.
        (root / "broken.c").write_text("int broken(void) { return 0; }\n", encoding="utf-8")
        (root / "compile_commands.json").write_text(json.dumps([{
            "directory": str(root),
            "arguments": ["clang", "-fno-such-lexicon-option", "-c", "broken.c"],
            "file": "broken.c",
        }]), encoding="utf-8")
        failing = {**request, "owned_files": ["broken.c"], "workers": 1}
        result = subprocess.run(
            [str(helper), "--protocol-version", "3", "--helper-version", version],
            input=(json.dumps(failing) + "\n").encode(), capture_output=True,
            cwd=root, env={**os.environ, "LEXICON_PERF": "1"}, timeout=30)
        assert result.returncode != 0, "driver-invalid TU unexpectedly succeeded"
        trace = [line for line in result.stderr.decode(errors="replace").splitlines()
                 if "phase=real" in line and "rank=0" in line]
        teardown = next(i for i, line in enumerate(trace)
                        if "stage=c-family.clang.teardown" in line)
        reclaim = next(i for i, line in enumerate(trace)
                       if "stage=c-family.clang.heap_reclaim" in line)
        admission = next(i for i, line in enumerate(trace)
                         if "stage=c-family.clang.ordered_admission_wait" in line)
        assert teardown < reclaim < admission, trace
        print("large AST reclamation, failed frontend, and worker determinism passed")

if __name__ == "__main__":
    verify(pathlib.Path(sys.argv[1]).resolve(), sys.argv[2])

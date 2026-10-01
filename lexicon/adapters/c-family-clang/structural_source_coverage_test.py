#!/usr/bin/env python3
"""Small native regressions for real-context source coverage."""
from __future__ import annotations

import json
import pathlib
import sys

from structural_ownership_test import (
    assert_metric, compile_database, run_with_perf,
    temporary_directory, write_compile_database,
)


def request(root, owned, workers):
    return {
        "protocol_version": 3, "operation": "structural",
        "repository_root": str(root), "owned_files": owned,
        "context_files": [], "workers": workers,
    }


def paths(response, field="files"):
    return [value["path"] for value in response[field]]


def unity_context_replaces_only_covered_sources(helper, version, baseline=None):
    with temporary_directory("lexicon-unity-coverage-") as raw:
        root = pathlib.Path(raw)
        (root / "shared.h").write_text(
            "#pragma once\nint api(void);\n", encoding="utf-8"
        )
        (root / "part.c").write_text(
            "#if UNITY_CONTEXT\n"
            "int required_context(void) { return api(); }\n"
            "#else\nint standalone_context(void) { return 1; }\n#endif\n",
            encoding="utf-8",
        )
        (root / "real.c").write_text(
            '#include "shared.h"\n#include "part.c"\n', encoding="utf-8"
        )
        (root / "other.c").write_text(
            '#include "shared.h"\nint other(void) { return api(); }\n',
            encoding="utf-8",
        )
        (root / "unbuilt.c").write_text(
            '#include "shared.h"\nint unbuilt(void) { return api(); }\n',
            encoding="utf-8",
        )
        (root / "orphan.h").write_text("int orphan(void);\n", encoding="utf-8")
        compile_database(root, ["real.c", "other.c"])
        records = json.loads((root / "compile_commands.json").read_text())
        for record in records:
            record["arguments"].insert(1, "-DUNITY_CONTEXT=1")
        write_compile_database(root, records)
        owned = ["real.c", "other.c", "part.c", "unbuilt.c", "shared.h", "orphan.h"]
        canonical = []
        for workers in (1, 2, 4):
            response, stderr = run_with_perf(
                helper, version, root, request(root, owned, workers)
            )
            assert paths(response) == sorted(owned), response
            assert set(paths(response, "translation_units")) == {
                "real.c", "other.c", "unbuilt.c", "orphan.h"
            }, response
            stage = "c-family.clang.execution"
            assert_metric(stderr, stage, "primary_synthetic_parse_units", 1)
            assert_metric(stderr, stage, "synthetic_source_candidates", 2)
            assert_metric(stderr, stage, "skipped_covered_source_units", 1)
            assert_metric(stderr, stage, "orphan_fallback_units", 1)
            assert_metric(stderr, stage, "completed_tus", 4)
            assert_metric(stderr, stage, "claimed_owned_files", len(owned))
            part = next(file for file in response["files"] if file["path"] == "part.c")
            assert part["translation_units"] == ["real.c"], part
            assert "required_context" in {d["name"] for d in part["declarations"]}
            assert "standalone_context" not in {d["name"] for d in part["declarations"]}
            assert part["calls"], part
            canonical.append(json.dumps(response, sort_keys=True))
        assert len(set(canonical)) == 1, "worker counts changed observations"
        if baseline:
            previous, _ = run_with_perf(
                baseline, version, root, request(root, owned, 2)
            )
            assert previous["files"] == response["files"], "owned semantic observations changed"


def real_command_is_always_parsed(helper, version):
    with temporary_directory("lexicon-explicit-source-") as raw:
        root = pathlib.Path(raw)
        (root / "part.c").write_text("int part(void) { return 1; }\n")
        (root / "real.c").write_text('#include "part.c"\n')
        compile_database(root, ["real.c", "part.c"])
        response, stderr = run_with_perf(
            helper, version, root, request(root, ["real.c", "part.c"], 2)
        )
        assert set(paths(response, "translation_units")) == {"real.c", "part.c"}
        assert_metric(stderr, "c-family.clang.execution", "completed_tus", 2)
        assert_metric(stderr, "c-family.clang.execution", "skipped_covered_source_units", 0)


def empty_or_inactive_sources_keep_fallback(helper, version):
    with temporary_directory("lexicon-empty-source-") as raw:
        root = pathlib.Path(raw)
        (root / "empty.c").write_text("/* empty */\n")
        (root / "inactive.c").write_text("int inactive(void) { return 3; }\n")
        (root / "real.c").write_text(
            '#include "empty.c"\n#if 0\n#include "inactive.c"\n#endif\n'
        )
        compile_database(root, ["real.c"])
        owned = ["real.c", "empty.c", "inactive.c"]
        response, stderr = run_with_perf(helper, version, root, request(root, owned, 2))
        assert paths(response) == sorted(owned)
        assert set(paths(response, "translation_units")) == set(owned)
        inactive = next(f for f in response["files"] if f["path"] == "inactive.c")
        assert any(d["name"] == "inactive" for d in inactive["declarations"])
        assert_metric(stderr, "c-family.clang.execution", "skipped_covered_source_units", 0)


def language_mismatch_keeps_fallback(helper, version):
    with temporary_directory("lexicon-language-source-") as raw:
        root = pathlib.Path(raw)
        (root / "part.c").write_text("int part(void) { return 1; }\n")
        (root / "real.cpp").write_text('#include "part.c"\n')
        compile_database(root, ["real.cpp"])
        response, stderr = run_with_perf(
            helper, version, root, request(root, ["real.cpp", "part.c"], 2)
        )
        assert set(paths(response, "translation_units")) == {"real.cpp", "part.c"}
        assert_metric(stderr, "c-family.clang.execution", "skipped_covered_source_units", 0)


def invalid_real_context_keeps_fallback(helper, version):
    with temporary_directory("lexicon-invalid-source-") as raw:
        root = pathlib.Path(raw)
        (root / "part.c").write_text("int part(void) { return 1; }\n")
        (root / "real.c").write_text('#include "part.c"\nint broken( {\n')
        compile_database(root, ["real.c"])
        # Recovery AST diagnostics need not cause tool exit failure, so inspect
        # the stream using the usual runner for the current recovery contract.
        response, stderr = run_with_perf(
            helper, version, root, request(root, ["real.c", "part.c"], 2)
        )
        assert set(paths(response, "translation_units")) == {"real.c", "part.c"}
        assert any(d["severity"] in {"error", "fatal"} for d in response["diagnostics"])
        assert_metric(stderr, "c-family.clang.execution", "skipped_covered_source_units", 0)


def main():
    helper = pathlib.Path(sys.argv[1]).resolve()
    version = sys.argv[2]
    baseline = pathlib.Path(sys.argv[3]).resolve() if len(sys.argv) > 3 else None
    unity_context_replaces_only_covered_sources(helper, version, baseline)
    real_command_is_always_parsed(helper, version)
    empty_or_inactive_sources_keep_fallback(helper, version)
    language_mismatch_keeps_fallback(helper, version)
    invalid_real_context_keeps_fallback(helper, version)
    print("staged source coverage: five fixtures passed (workers 1/2/4)")


if __name__ == "__main__":
    main()

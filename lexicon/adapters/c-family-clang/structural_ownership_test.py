#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import pathlib
import subprocess
import sys
import tempfile

REPOSITORY_ROOT = pathlib.Path(__file__).resolve().parents[3]


def temporary_directory(prefix: str) -> tempfile.TemporaryDirectory:
    return tempfile.TemporaryDirectory(prefix=prefix, dir=REPOSITORY_ROOT)


def decode_framed_response(stdout: bytes) -> dict:
    offset = 0
    metadata = None
    files = []
    while offset < len(stdout):
        header_end = stdout.find(b"\n", offset)
        if header_end < 0:
            raise RuntimeError("framed response header is not newline terminated")
        header = json.loads(stdout[offset:header_end].decode("utf-8"))
        offset = header_end + 1
        payload_bytes = int(header["bytes"])
        payload_end = offset + payload_bytes
        if payload_end >= len(stdout) or stdout[payload_end : payload_end + 1] != b"\n":
            raise RuntimeError("framed response payload is truncated")
        payload = json.loads(stdout[offset:payload_end].decode("utf-8"))
        offset = payload_end + 1
        if header["kind"] == "metadata":
            if metadata is not None:
                raise RuntimeError("duplicate metadata frame")
            metadata = payload
        elif header["kind"] == "file":
            if payload.get("path") != header.get("path"):
                raise RuntimeError("file frame header/payload path mismatch")
            files.append(payload)
        else:
            raise RuntimeError(f"unexpected frame kind: {header['kind']!r}")
    if metadata is None:
        raise RuntimeError("framed response omitted metadata")
    files.sort(key=lambda value: value["path"])
    return {**metadata, "files": files}


def run(helper: pathlib.Path, version: str, root: pathlib.Path, request: dict) -> dict:
    completed = subprocess.run(
        [str(helper), "--protocol-version", "3", "--helper-version", version],
        input=(json.dumps(request) + "\n").encode("utf-8"),
        capture_output=True,
        cwd=root,
        timeout=120,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"helper failed ({completed.returncode}): "
            f"{completed.stderr.decode('utf-8', errors='replace')}\n"
            f"{completed.stdout.decode('utf-8', errors='replace')}"
        )
    return decode_framed_response(completed.stdout)


def run_with_perf(
    helper: pathlib.Path,
    version: str,
    root: pathlib.Path,
    request: dict,
) -> tuple[dict, str]:
    environment = os.environ.copy()
    environment["LEXICON_PERF"] = "1"
    completed = subprocess.run(
        [str(helper), "--protocol-version", "3", "--helper-version", version],
        input=(json.dumps(request) + "\n").encode("utf-8"),
        capture_output=True,
        cwd=root,
        env=environment,
        timeout=120,
    )
    stderr = completed.stderr.decode("utf-8", errors="replace")
    if completed.returncode != 0:
        raise RuntimeError(
            f"helper failed ({completed.returncode}): {stderr}\n"
            f"{completed.stdout.decode('utf-8', errors='replace')}"
        )
    return decode_framed_response(completed.stdout), stderr


def perf_totals(stderr: str, stage: str) -> dict[str, int]:
    totals: dict[str, int] = {}
    lines = [line for line in stderr.splitlines() if f"stage={stage}" in line]
    if not lines:
        raise RuntimeError(f"missing {stage} perf metrics")
    for line in lines:
        for token in line.split():
            if "=" not in token:
                continue
            key, value = token.split("=", 1)
            try:
                totals[key] = totals.get(key, 0) + int(value)
            except ValueError:
                continue
    return totals


def assert_metric(stderr: str, stage: str, name: str, expected: int) -> None:
    actual = perf_totals(stderr, stage).get(name)
    if actual != expected:
        raise RuntimeError(
            f"expected {stage} {name}={expected}, got {actual!r}: {stderr!r}"
        )

def compile_database(root: pathlib.Path, files: list[str]) -> None:
    entries = [
        {
            "directory": str(root),
            "arguments": ["clang", "-I", str(root), "-c", name],
            "file": name,
        }
        for name in files
    ]
    (root / "compile_commands.json").write_text(json.dumps(entries), encoding="utf-8")


def write_compile_database(root: pathlib.Path, entries: list[dict]) -> None:
    (root / "compile_commands.json").write_text(
        json.dumps(entries), encoding="utf-8"
    )


def run_expect_failure(
    helper: pathlib.Path,
    version: str,
    root: pathlib.Path,
    request: dict,
    *,
    protocol_version: int = 3,
    timeout: int = 30,
) -> subprocess.CompletedProcess:
    completed = subprocess.run(
        [
            str(helper),
            "--protocol-version",
            str(protocol_version),
            "--helper-version",
            version,
        ],
        input=(json.dumps(request) + "\n").encode("utf-8"),
        capture_output=True,
        cwd=root,
        timeout=timeout,
    )
    if completed.returncode == 0:
        raise RuntimeError(
            "helper unexpectedly accepted an invalid request: "
            f"{completed.stdout.decode('utf-8', errors='replace')}"
        )
    return completed


def changed_source_only(helper: pathlib.Path, version: str) -> None:
    with temporary_directory("lexicon-changed-source-") as temp:
        root = pathlib.Path(temp)
        (root / "api.h").write_text(
            "#pragma once\nint api(void);\n", encoding="utf-8"
        )
        for name in ("changed.c", "unchanged.c"):
            (root / name).write_text(
                '#include "api.h"\n'
                f"int {name.removesuffix('.c')}(void) {{ return api(); }}\n",
                encoding="utf-8",
            )
        compile_database(root, ["changed.c", "unchanged.c"])
        response = run(
            helper,
            version,
            root,
            {
                "protocol_version": 3,
                "operation": "structural",
                "repository_root": str(root),
                "owned_files": ["changed.c"],
                "context_files": ["api.h", "unchanged.c"],
                "workers": 2,
            },
        )
        paths = [value["path"] for value in response.get("files", [])]
        if paths != ["changed.c"]:
            raise RuntimeError(
                f"source-only incremental request emitted {paths!r}"
            )
        units = response.get("translation_units", [])
        if [value.get("path") for value in units] != ["changed.c"]:
            raise RuntimeError(
                f"source-only request parsed unchanged context TUs: {units!r}"
            )


def changed_header_uses_real_context_first(
    helper: pathlib.Path, version: str
) -> None:
    with temporary_directory("lexicon-changed-header-") as temp:
        root = pathlib.Path(temp)
        (root / "changed.h").write_text(
            "#pragma once\n"
            "#if REAL_CONTEXT\n"
            "int from_real_context(void);\n"
            "#else\n"
            "int from_synthetic_context(void);\n"
            "#endif\n",
            encoding="utf-8",
        )
        (root / "real.c").write_text(
            '#include "changed.h"\nint real_user(void) { return 0; }\n',
            encoding="utf-8",
        )
        (root / "synthetic.c").write_text(
            '#include "changed.h"\nint synthetic_user(void) { return 0; }\n',
            encoding="utf-8",
        )
        (root / "compile_commands.json").write_text(
            json.dumps(
                [
                    {
                        "directory": str(root),
                        "arguments": [
                            "clang",
                            "-DREAL_CONTEXT=1",
                            "-I",
                            str(root),
                            "-c",
                            "real.c",
                        ],
                        "file": "real.c",
                    }
                ]
            ),
            encoding="utf-8",
        )
        response = run(
            helper,
            version,
            root,
            {
                "protocol_version": 3,
                "operation": "structural",
                "repository_root": str(root),
                "owned_files": ["changed.h"],
                "context_files": ["real.c", "synthetic.c"],
                "workers": 2,
            },
        )
        files = response.get("files", [])
        if len(files) != 1 or files[0].get("path") != "changed.h":
            raise RuntimeError(f"changed-header output was not singular: {files!r}")
        names = {value.get("name") for value in files[0].get("declarations", [])}
        if "from_real_context" not in names or "from_synthetic_context" in names:
            raise RuntimeError(
                "real compile-command context did not win over synthetic context: "
                f"{names!r}"
            )
        units = response.get("translation_units", [])
        by_path = {value.get("path"): value for value in units}
        if set(by_path) != {"real.c", "synthetic.c"}:
            raise RuntimeError(f"missing real/synthetic header candidates: {units!r}")
        if by_path["real.c"].get("synthesized") or not by_path[
            "synthetic.c"
        ].get("synthesized"):
            raise RuntimeError(f"candidate compile contexts were misclassified: {units!r}")
        if files[0].get("translation_units") != ["real.c"]:
            raise RuntimeError(
                "owned header was not claimed by the earliest real TU: "
                f"{files[0].get('translation_units')!r}"
            )


def orphan_header_is_fallback_parsed_once(
    helper: pathlib.Path, version: str
) -> None:
    with temporary_directory("lexicon-orphan-header-") as temp:
        root = pathlib.Path(temp)
        (root / "orphan.h").write_text(
            "#pragma once\nint orphan_api(void);\n", encoding="utf-8"
        )
        compile_database(root, [])
        response, stderr = run_with_perf(
            helper,
            version,
            root,
            {
                "protocol_version": 3,
                "operation": "structural",
                "repository_root": str(root),
                "owned_files": ["orphan.h"],
                "context_files": [],
                "workers": 1,
            },
        )
        files = response.get("files", [])
        if [value.get("path") for value in files] != ["orphan.h"]:
            raise RuntimeError(f"orphan header was not emitted exactly once: {files!r}")
        units = response.get("translation_units", [])
        if [value.get("path") for value in units] != ["orphan.h"]:
            raise RuntimeError(f"orphan header fallback was not parsed once: {units!r}")
        assert_metric(stderr, "c-family.clang.execution", "orphan_fallback_units", 1)


def synthetic_source_is_parsed_once(helper: pathlib.Path, version: str) -> None:
    with temporary_directory("lexicon-synthetic-source-") as temp:
        root = pathlib.Path(temp)
        (root / "synthetic.c").write_text(
            "int synthetic_source(void) { return 7; }\n", encoding="utf-8"
        )
        compile_database(root, [])
        response, stderr = run_with_perf(
            helper,
            version,
            root,
            {
                "protocol_version": 3,
                "operation": "structural",
                "repository_root": str(root),
                "owned_files": ["synthetic.c"],
                "context_files": [],
                "workers": 1,
            },
        )
        units = response.get("translation_units", [])
        if len(units) != 1 or units[0].get("path") != "synthetic.c":
            raise RuntimeError(f"synthetic source was not parsed once: {units!r}")
        if not units[0].get("synthesized"):
            raise RuntimeError(f"source without a command was not synthetic: {units!r}")
        if [value.get("path") for value in response.get("files", [])] != [
            "synthetic.c"
        ]:
            raise RuntimeError("synthetic source facts were not emitted exactly once")
        assert_metric(stderr, "c-family.clang.execution", "completed_tus", 1)
        assert_metric(
            stderr, "c-family.clang.execution", "primary_synthetic_parse_units", 1
        )


def empty_owned_source_is_emitted_once(helper: pathlib.Path, version: str) -> None:
    with temporary_directory("lexicon-empty-source-") as temp:
        root = pathlib.Path(temp)
        (root / "empty.c").write_text("", encoding="utf-8")
        compile_database(root, ["empty.c"])
        response = run(
            helper,
            version,
            root,
            {
                "protocol_version": 3,
                "operation": "structural",
                "repository_root": str(root),
                "owned_files": ["empty.c"],
                "context_files": [],
                "workers": 1,
            },
        )
        files = response.get("files", [])
        if [value.get("path") for value in files] != ["empty.c"]:
            raise RuntimeError(
                f"empty owned source was not emitted exactly once: {files!r}"
            )


def differing_compile_directories_are_respected(
    helper: pathlib.Path, version: str
) -> None:
    with temporary_directory("lexicon-compile-directories-") as temp:
        root = pathlib.Path(temp)
        build_dirs = {name: root / name / "build" for name in ("one", "two")}
        for name, build_dir in build_dirs.items():
            source_dir = root / name / "src"
            include_dir = root / name / "include"
            build_dir.mkdir(parents=True)
            source_dir.mkdir()
            include_dir.mkdir()
            (include_dir / "api.h").write_text(
                f"int {name}_api(void);\n", encoding="utf-8"
            )
            (source_dir / "main.c").write_text(
                '#include "include/api.h"\n'
                f"int {name}_user(void) {{ return {name}_api(); }}\n",
                encoding="utf-8",
            )
        write_compile_database(
            root,
            [
                {
                    "directory": str(build_dirs["two"]),
                    "arguments": ["clang", "-I", "..", "-c", "../src/main.c"],
                    "file": "../src/main.c",
                },
                {
                    "directory": str(build_dirs["one"]),
                    "arguments": ["clang", "-I", "..", "-c", "../src/main.c"],
                    "file": "../src/main.c",
                },
            ],
        )
        response = run(
            helper,
            version,
            root,
            {
                "protocol_version": 3,
                "operation": "structural",
                "repository_root": str(root),
                "owned_files": [
                    "one/include/api.h",
                    "one/src/main.c",
                    "two/include/api.h",
                    "two/src/main.c",
                ],
                "context_files": [],
                "workers": 1,
            },
        )
        files = response.get("files", [])
        expected_files = [
            "one/include/api.h",
            "one/src/main.c",
            "two/include/api.h",
            "two/src/main.c",
        ]
        if [value.get("path") for value in files] != expected_files:
            raise RuntimeError(
                f"different command directories lost owned source output: {files!r}"
            )
        units = {
            value.get("path"): value
            for value in response.get("translation_units", [])
        }
        expected_directories = {
            "one/src/main.c": str(build_dirs["one"]),
            "two/src/main.c": str(build_dirs["two"]),
        }
        if set(units) != set(expected_directories) or any(
            pathlib.Path(units[path].get("directory", "")).resolve()
            != pathlib.Path(directory).resolve()
            for path, directory in expected_directories.items()
        ):
            raise RuntimeError(
                f"translation units did not retain their compile command directories: {units!r}"
            )
        declarations = {
            value.get("path"): {
                item.get("name") for item in value.get("declarations", [])
            }
            for value in files
        }
        if "one_api" not in declarations["one/include/api.h"] or (
            "two_api" in declarations["one/include/api.h"]
        ) or "two_api" not in declarations["two/include/api.h"] or (
            "one_api" in declarations["two/include/api.h"]
        ):
            raise RuntimeError(
                f"relative header paths collided between command directories: {declarations!r}"
            )


def skipped_driver_tu_does_not_block_later_tu(
    helper: pathlib.Path, version: str
) -> None:
    with temporary_directory("lexicon-skipped-driver-tu-") as temp:
        root = pathlib.Path(temp)
        (root / "a-skipped.c").write_text(
            "int skipped_source(void) { return 0; }\n", encoding="utf-8"
        )
        (root / "z-valid.c").write_text(
            "int later_valid_source(void) { return 42; }\n", encoding="utf-8"
        )
        write_compile_database(
            root,
            [
                {
                    "directory": str(root),
                    "arguments": ["clang", "-fno-such-lexicon-option", "-c", "a-skipped.c"],
                    "file": "a-skipped.c",
                },
                {
                    "directory": str(root),
                    "arguments": ["clang", "-c", "z-valid.c"],
                    "file": "z-valid.c",
                },
            ],
        )
        completed = subprocess.run(
            [str(helper), "--protocol-version", "3", "--helper-version", version],
            input=(
                json.dumps(
                    {
                        "protocol_version": 3,
                        "operation": "structural",
                        "repository_root": str(root),
                        "owned_files": ["a-skipped.c", "z-valid.c"],
                        "context_files": [],
                        "workers": 1,
                    }
                )
                + "\n"
            ).encode("utf-8"),
            capture_output=True,
            cwd=root,
            timeout=30,
        )
        if completed.returncode == 0:
            raise RuntimeError("driver-skipped translation unit did not fail the request")
        if not completed.stdout:
            raise RuntimeError(
                "driver-skipped translation unit prevented framed results from being emitted"
            )
        response = decode_framed_response(completed.stdout)
        emitted = [value.get("path") for value in response.get("files", [])]
        units = {value.get("path") for value in response.get("translation_units", [])}
        if "z-valid.c" not in emitted or "z-valid.c" not in units:
            raise RuntimeError(
                "valid TU after a driver-skipped rank was not completed and emitted: "
                f"files={emitted!r}, units={units!r}"
            )


def protocol_v3_hard_cut_is_enforced(helper: pathlib.Path, version: str) -> None:
    with temporary_directory("lexicon-protocol-v3-") as temp:
        root = pathlib.Path(temp)
        compile_database(root, [])
        base = {
            "protocol_version": 3,
            "operation": "structural",
            "repository_root": str(root),
            "owned_files": [],
            "context_files": [],
            "workers": 1,
        }
        run_expect_failure(
            helper,
            version,
            root,
            {**base, "protocol_version": 2},
            protocol_version=2,
        )
        run_expect_failure(
            helper,
            version,
            root,
            {**base, "shards": 1, "merge_fan_in": 1},
        )


def failed_frontend_exits_without_hanging(helper: pathlib.Path, version: str) -> None:
    with temporary_directory("lexicon-frontend-failure-") as temp:
        root = pathlib.Path(temp)
        (root / "broken.c").write_text("int broken(void) { return 0; }\n", encoding="utf-8")
        write_compile_database(
            root,
            [
                {
                    "directory": str(root),
                    "arguments": ["clang", "-fno-such-lexicon-option", "-c", "broken.c"],
                    "file": "broken.c",
                }
            ],
        )
        try:
            completed = subprocess.run(
                [
                    str(helper),
                    "--protocol-version",
                    "3",
                    "--helper-version",
                    version,
                ],
                input=(
                    json.dumps(
                        {
                            "protocol_version": 3,
                            "operation": "structural",
                            "repository_root": str(root),
                            "owned_files": ["broken.c"],
                            "context_files": [],
                            "workers": 2,
                        }
                    )
                    + "\n"
                ).encode("utf-8"),
                capture_output=True,
                cwd=root,
                timeout=30,
            )
        except subprocess.TimeoutExpired as error:
            raise RuntimeError("failed frontend hung instead of completing") from error
        if completed.returncode == 0:
            raise RuntimeError("driver-invalid frontend unexpectedly succeeded")


def syntax_error_is_observed_without_aborting(
    helper: pathlib.Path, version: str
) -> None:
    with temporary_directory("lexicon-syntax-diagnostic-") as temp:
        root = pathlib.Path(temp)
        (root / "broken.c").write_text("int broken( {\n", encoding="utf-8")
        compile_database(root, ["broken.c"])
        response = run(
            helper,
            version,
            root,
            {
                "protocol_version": 3,
                "operation": "structural",
                "repository_root": str(root),
                "owned_files": ["broken.c"],
                "context_files": [],
                "workers": 1,
            },
        )
        if [value.get("path") for value in response.get("files", [])] != ["broken.c"]:
            raise RuntimeError("syntax-error TU did not retain owned-file observations")
        if not any(
            value.get("severity") in {"error", "fatal"}
            for value in response.get("diagnostics", [])
        ):
            raise RuntimeError("syntax-error TU emitted no compiler diagnostic")


def shared_header_once(helper: pathlib.Path, version: str) -> None:
    with temporary_directory("lexicon-ownership-") as temp:
        root = pathlib.Path(temp)
        (root / "shared.h").write_text(
            "#pragma once\nint shared_value(void);\n", encoding="utf-8"
        )
        sources = []
        for index in range(50):
            name = f"tu{index:02d}.c"
            sources.append(name)
            (root / name).write_text(
                '#include "shared.h"\nint use_shared(void) { return shared_value(); }\n',
                encoding="utf-8",
            )
        compile_database(root, sources)
        request = {
            "protocol_version": 3,
            "operation": "structural",
            "repository_root": str(root),
            "owned_files": ["shared.h"],
            "context_files": sources,
            "workers": 4,
        }
        response, stderr = run_with_perf(
            helper,
            version,
            root,
            request,
        )
        paths = [value["path"] for value in response.get("files", [])]
        if paths != ["shared.h"]:
            raise RuntimeError(
                f"shared header ownership leaked or duplicated: emitted {paths!r}"
            )
        units = response.get("translation_units", [])
        if len(units) != 50 or {value.get("path") for value in units} != set(sources):
            raise RuntimeError(f"expected 50 real source TUs, got {units!r}")
        if any(value.get("synthesized") for value in units):
            raise RuntimeError("real compile-database TUs were synthesized")
        assert_metric(stderr, "c-family.clang.execution", "active_clang_lanes", 4)
        assert_metric(stderr, "c-family.clang.execution", "completed_tus", 50)
        assert_metric(stderr, "c-family.clang.execution", "claimed_owned_files", 1)
        assert_metric(
            stderr,
            "c-family.clang.execution",
            "discarded_duplicate_file_observations",
            49,
        )


def context_identity_without_context_file(
    helper: pathlib.Path, version: str
) -> None:
    with temporary_directory("lexicon-context-") as temp:
        root = pathlib.Path(temp)
        (root / "api.h").write_text(
            "#pragma once\nint api(void);\n", encoding="utf-8"
        )
        (root / "owner.c").write_text(
            '#include "api.h"\nint owner(void) { return api() + api(); }\n',
            encoding="utf-8",
        )
        compile_database(root, ["owner.c"])
        response = run(
            helper,
            version,
            root,
            {
                "protocol_version": 3,
                "operation": "structural",
                "repository_root": str(root),
                "owned_files": ["owner.c"],
                "context_files": ["api.h"],
                "workers": 1,
            },
        )
        paths = [value["path"] for value in response.get("files", [])]
        if paths != ["owner.c"]:
            raise RuntimeError(f"context file became owned: emitted {paths!r}")
        identities = response.get("context_identities", [])
        api_identities = [
            value
            for value in identities
            if value.get("path") == "api.h"
            and value.get("qualified_name") == "api"
            and value.get("kind") == "function"
        ]
        if len(api_identities) != 1:
            raise RuntimeError(
                f"expected one compact context identity for api.h::api: {identities!r}"
            )



def relationship_sources_are_materialized(
    helper: pathlib.Path, version: str
) -> None:
    with temporary_directory("lexicon-relationship-source-") as temp:
        root = pathlib.Path(temp)
        (root / "main.cpp").write_text(
            "struct Base { virtual int run() { return 1; } };\n"
            "struct Derived : Base { int run() override { return 2; } };\n"
            "int use() { Derived value; return value.run(); }\n",
            encoding="utf-8",
        )
        compile_database(root, ["main.cpp"])
        response = run(
            helper,
            version,
            root,
            {
                "protocol_version": 3,
                "operation": "structural",
                "repository_root": str(root),
                "owned_files": ["main.cpp"],
                "context_files": [],
                "workers": 1,
            },
        )
        files = response.get("files", [])
        if len(files) != 1 or files[0].get("path") != "main.cpp":
            raise RuntimeError(f"unexpected relationship fixture output: {files!r}")
        declarations = {
            value["compiler_id"] for value in files[0].get("declarations", [])
        }
        relationships = files[0].get("relationships", [])
        if not relationships:
            raise RuntimeError("relationship fixture emitted no relationships")
        missing = sorted(
            {
                value["source_compiler_id"]
                for value in relationships
                if value["source_compiler_id"] not in declarations
            }
        )
        if missing:
            raise RuntimeError(
                "relationship sources were not materialized as owned declarations: "
                f"{missing!r}"
            )


def execution_policy_is_fact_stable(helper: pathlib.Path, version: str) -> None:
    with temporary_directory("lexicon-execution-policy-") as temp:
        root = pathlib.Path(temp)
        (root / "shared.h").write_text(
            "#pragma once\nstatic inline int shared_value(int value) { return value + 1; }\n",
            encoding="utf-8",
        )
        (root / "api.h").write_text(
            "#pragma once\nint api(void);\n", encoding="utf-8"
        )

        sources = []
        for index in range(9):
            name = f"unit{index:02d}.c"
            sources.append(name)
            (root / name).write_text(
                '#include "shared.h"\n'
                '#include "api.h"\n'
                f"int unit{index:02d}(void) {{ return shared_value({index}) + api(); }}\n",
                encoding="utf-8",
            )
        compile_database(root, sources)

        base_request = {
            "protocol_version": 3,
            "operation": "structural",
            "repository_root": str(root),
            "owned_files": ["shared.h", *sources],
            "context_files": ["api.h"],
        }
        configurations = [1, 2, 4]
        canonical = []
        for workers in configurations:
            response, stderr = run_with_perf(
                helper,
                version,
                root,
                {
                    **base_request,
                    "workers": workers,
                },
            )
            assert_metric(
                stderr, "c-family.clang.execution", "active_clang_lanes", workers
            )
            assert_metric(stderr, "c-family.clang.execution", "completed_tus", 9)
            expected_paths = sorted(["shared.h", *sources])
            actual_paths = [value.get("path") for value in response.get("files", [])]
            if actual_paths != expected_paths:
                raise RuntimeError(
                    "full-scan owned files were not each emitted exactly once: "
                    f"{actual_paths!r}"
                )
            emission = next(
                (
                    line
                    for line in stderr.splitlines()
                    if "stage=c-family.clang.observation_emission" in line
                ),
                None,
            )
            if emission is None:
                raise RuntimeError("missing observation-emission perf metrics")
            counters = {
                token.split("=", 1)[0]: token.split("=", 1)[1]
                for token in emission.split()
                if "=" in token
            }
            for counter in ("transport_frames", "transport_bytes", "peak_rss_bytes"):
                if int(counters.get(counter, "0")) <= 0:
                    raise RuntimeError(
                        f"missing or zero {counter} in emission metrics: {emission!r}"
                    )
            hot_path_lines = [
                line
                for line in stderr.splitlines()
                if "stage=c-family.clang.hot_path" in line
            ]
            if not hot_path_lines:
                raise RuntimeError("missing C-family hot-path profile metrics")
            hot_totals: dict[str, int] = {}
            for line in hot_path_lines:
                for token in line.split():
                    if "=" not in token:
                        continue
                    key, value = token.split("=", 1)
                    if key in {"stage", "elapsed_ms"}:
                        continue
                    try:
                        hot_totals[key] = hot_totals.get(key, 0) + int(value)
                    except ValueError:
                        pass
            for counter in (
                "repository_path_cache_hits",
                "repository_path_cache_misses",
                "compiler_id_cache_hits",
                "compiler_id_cache_misses",
                "compiler_id_ns",
            ):
                if hot_totals.get(counter, 0) <= 0:
                    raise RuntimeError(
                        f"missing or zero {counter} in hot-path metrics: "
                        f"{hot_path_lines!r}"
                    )
            canonical.append(
                json.dumps(
                    response,
                    sort_keys=True,
                    separators=(",", ":"),
                    ensure_ascii=False,
                ).encode("utf-8")
            )

        if any(value != canonical[0] for value in canonical[1:]):
            raise RuntimeError(
                "execution-policy variants changed canonical structural facts"
            )


def main() -> int:
    helper = pathlib.Path(sys.argv[1]).resolve()
    version = sys.argv[2]
    if version != "0.7.0":
        raise RuntimeError(f"native ownership regressions require helper 0.7.0, got {version}")
    shared_header_once(helper, version)
    changed_source_only(helper, version)
    changed_header_uses_real_context_first(helper, version)
    orphan_header_is_fallback_parsed_once(helper, version)
    synthetic_source_is_parsed_once(helper, version)
    empty_owned_source_is_emitted_once(helper, version)
    differing_compile_directories_are_respected(helper, version)
    skipped_driver_tu_does_not_block_later_tu(helper, version)
    context_identity_without_context_file(helper, version)
    relationship_sources_are_materialized(helper, version)
    execution_policy_is_fact_stable(helper, version)
    protocol_v3_hard_cut_is_enforced(helper, version)
    failed_frontend_exits_without_hanging(helper, version)
    syntax_error_is_observed_without_aborting(helper, version)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

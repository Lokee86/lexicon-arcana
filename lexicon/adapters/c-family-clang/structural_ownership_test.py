#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import pathlib
import subprocess
import sys
import tempfile


def run(helper: pathlib.Path, version: str, root: pathlib.Path, request: dict) -> dict:
    completed = subprocess.run(
        [str(helper), "--protocol-version", "2", "--helper-version", version],
        input=json.dumps(request),
        text=True,
        capture_output=True,
        cwd=root,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"helper failed ({completed.returncode}): {completed.stderr}\n{completed.stdout}"
        )
    return json.loads(completed.stdout)



def run_with_perf(
    helper: pathlib.Path,
    version: str,
    root: pathlib.Path,
    request: dict,
    legacy_jobs: int,
) -> tuple[dict, str]:
    environment = os.environ.copy()
    environment["LEXICON_PERF"] = "1"
    environment["LEXICON_CLANG_JOBS"] = str(legacy_jobs)
    completed = subprocess.run(
        [str(helper), "--protocol-version", "2", "--helper-version", version],
        input=json.dumps(request),
        text=True,
        capture_output=True,
        cwd=root,
        env=environment,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"helper failed ({completed.returncode}): {completed.stderr}\n{completed.stdout}"
        )
    return json.loads(completed.stdout), completed.stderr


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


def shared_header_once(helper: pathlib.Path, version: str) -> None:
    with tempfile.TemporaryDirectory(prefix="lexicon-ownership-") as temp:
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
        response = run(
            helper,
            version,
            root,
            {
                "protocol_version": 2,
                "operation": "structural",
                "repository_root": str(root),
                "owned_files": ["shared.h"],
                "context_files": sources,
                "workers": 4,
                "shards": 16,
                "merge_fan_in": 4,
            },
        )
        paths = [value["path"] for value in response.get("files", [])]
        if paths != ["shared.h"]:
            raise RuntimeError(
                f"shared header ownership leaked or duplicated: emitted {paths!r}"
            )


def context_identity_without_context_file(
    helper: pathlib.Path, version: str
) -> None:
    with tempfile.TemporaryDirectory(prefix="lexicon-context-") as temp:
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
                "protocol_version": 2,
                "operation": "structural",
                "repository_root": str(root),
                "owned_files": ["owner.c"],
                "context_files": ["api.h"],
                "workers": 1,
                "shards": 1,
                "merge_fan_in": 2,
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



def execution_policy_is_fact_stable(helper: pathlib.Path, version: str) -> None:
    with tempfile.TemporaryDirectory(prefix="lexicon-execution-policy-") as temp:
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
            "protocol_version": 2,
            "operation": "structural",
            "repository_root": str(root),
            "owned_files": ["shared.h", *sources],
            "context_files": ["api.h"],
        }
        configurations = [
            (1, 1, 2, 99, 1, 1),
            (2, 3, 2, 1, 3, 2),
            (4, 7, 4, 2, 7, 4),
        ]
        canonical = []
        for (
            workers,
            shards,
            merge_fan_in,
            legacy_jobs,
            expected_shards,
            expected_workers,
        ) in configurations:
            response, stderr = run_with_perf(
                helper,
                version,
                root,
                {
                    **base_request,
                    "workers": workers,
                    "shards": shards,
                    "merge_fan_in": merge_fan_in,
                },
                legacy_jobs,
            )
            expected_policy = (
                f"logical_shards={expected_shards} "
                f"worker_limit={expected_workers} "
                f"merge_fan_in={merge_fan_in}"
            )
            if expected_policy not in stderr:
                raise RuntimeError(
                    "helper did not report requested Lexicon execution policy: "
                    f"{expected_policy!r} not found in {stderr!r}"
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
    shared_header_once(helper, version)
    context_identity_without_context_file(helper, version)
    execution_policy_is_fact_stable(helper, version)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
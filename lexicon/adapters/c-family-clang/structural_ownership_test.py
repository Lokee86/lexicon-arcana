#!/usr/bin/env python3
from __future__ import annotations

import json
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


def main() -> int:
    helper = pathlib.Path(sys.argv[1]).resolve()
    version = sys.argv[2]
    shared_header_once(helper, version)
    context_identity_without_context_file(helper, version)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
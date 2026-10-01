#!/usr/bin/env python3
"""Generate compile_commands.json from a Makefile dry run."""

from __future__ import annotations

import argparse
import json
import shlex
import subprocess
from pathlib import Path

SOURCE_SUFFIXES = {".c", ".cc", ".cp", ".cpp", ".cxx", ".c++"}
COMPILERS = {
    "cc",
    "c++",
    "gcc",
    "g++",
    "clang",
    "clang++",
    "clang-18",
    "clang++-18",
}
OPTIONS_WITH_VALUES = {
    "-include",
    "-imacros",
    "-isystem",
    "-iquote",
    "-idirafter",
    "-isysroot",
    "--sysroot",
    "-target",
    "--target",
    "-x",
    "-std",
}


def compiler_index(tokens: list[str]) -> int | None:
    for offset, token in enumerate(tokens):
        if Path(token).name in COMPILERS:
            return offset
    return None


def is_source(token: str) -> bool:
    return Path(token).suffix.lower() in SOURCE_SUFFIXES


def source_path(source: str, repository: Path) -> str:
    path = Path(source)
    if not path.is_absolute():
        path = repository / path
    return str(path.resolve())


def syntax_only_command(
    compiler: str, tokens: list[str], source: str
) -> str:
    result = [compiler]
    index = 0
    while index < len(tokens):
        token = tokens[index]
        if token == "-o":
            index += 2
            continue
        if token in {"-c", "-S", "-E"}:
            index += 1
            continue
        if is_source(token):
            index += 1
            continue
        suffix = Path(token).suffix.lower()
        if suffix in {".o", ".obj", ".a", ".lib", ".so", ".dylib", ".dll"}:
            index += 1
            continue
        if token.startswith("-l") or token.startswith("-Wl,"):
            index += 1
            continue
        if token in {"-shared", "-static", "-rdynamic", "-pie"}:
            index += 1
            continue
        result.append(token)
        if token in OPTIONS_WITH_VALUES and index + 1 < len(tokens):
            result.append(tokens[index + 1])
            index += 2
            continue
        index += 1
    result.extend(["-fsyntax-only", source])
    return shlex.join(result)


def compile_records(line: str, repository: Path) -> list[dict]:
    # Git's quiet Make rules emit shell sequences such as
    # "echo '   ' CC git.o;clang ...". Inspect each command segment so the
    # compiler invocation is not hidden behind presentation plumbing.
    if ";" in line:
        records: list[dict] = []
        for segment in line.split(";"):
            records.extend(compile_records(segment.strip(), repository))
        return records

    try:
        tokens = shlex.split(line, posix=True)
    except ValueError:
        return []

    driver = compiler_index(tokens)
    if driver is None:
        return []
    compiler = tokens[driver]
    arguments = tokens[driver + 1 :]
    sources = [token for token in arguments if is_source(token)]
    if not sources:
        return []

    # Preserve ordinary one-source compilation commands verbatim. ClangTool
    # already understands compile_commands entries with -c/-o.
    if "-c" in arguments and len(sources) == 1:
        source = sources[0]
        return [
            {
                "directory": str(repository),
                "command": shlex.join(tokens[driver:]),
                "file": source_path(source, repository),
            }
        ]

    # Some Makefiles compile many C/C++ files directly in the final driver/link
    # invocation. Split that one command into independent syntax-only entries so
    # LibTooling receives the real -I/-D/-std build context without linking.
    records = []
    for source in sources:
        records.append(
            {
                "directory": str(repository),
                "command": syntax_only_command(compiler, arguments, source),
                "file": source_path(source, repository),
            }
        )
    return records


def compile_output_records(output: str, repository: Path) -> list[dict]:
    # Make preserves backslash-newline pairs in continued recipes. Remove
    # them as the shell would before tokenizing complete compiler commands.
    output = output.replace("\\\r\n", "").replace("\\\n", "")
    records: list[dict] = []
    seen: set[tuple[str, str]] = set()
    for line in output.splitlines():
        for record in compile_records(line, repository):
            key = (record["file"], record["command"])
            if key in seen:
                continue
            seen.add(key)
            records.append(record)
    return records


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--makefile", default="Makefile")
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--make-arg", action="append", default=[])
    args = parser.parse_args()

    repository = args.repository.resolve()
    command = [
        "make",
        "-n",
        "-B",
        "-j1",
        "-f",
        args.makefile,
        args.target,
        *args.make_arg,
    ]
    completed = subprocess.run(
        command,
        cwd=repository,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if completed.returncode != 0:
        raise SystemExit(
            f"make dry-run failed ({completed.returncode}):\n{completed.stderr}"
        )

    records = compile_output_records(completed.stdout, repository)

    if not records:
        raise SystemExit("make dry-run produced no C/C++ compile commands")

    output = args.output or repository / "compile_commands.json"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(records, indent=2) + "\n", encoding="utf-8")
    print(f"{len(records)} compile commands -> {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Build the compiler-backed Java adapter and its private runtime."""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import tempfile
from pathlib import Path


def executable(name: str) -> str:
    return name + ".exe" if os.name == "nt" else name


def find_jdk(repo: Path) -> Path:
    candidates = [
        os.environ.get("LEXICON_JDK_HOME"),
        os.environ.get("JAVA_HOME"),
        str(repo / ".tools" / "jdk"),
    ]
    for value in candidates:
        if not value:
            continue
        root = Path(value).resolve()
        if (root / "bin" / executable("javac")).is_file():
            return root
    javac = shutil.which("javac")
    if javac:
        return Path(javac).resolve().parent.parent
    raise FileNotFoundError(
        "JDK 21+ not found; set LEXICON_JDK_HOME/JAVA_HOME or run tools/bootstrap_jdk.py"
    )


def run(command: list[str], cwd: Path) -> None:
    print("+", " ".join(command))
    subprocess.run(command, cwd=cwd, check=True)


def build_java_adapter(repo: Path, output: Path) -> None:
    adapter = repo / "adapters" / "java"
    jdk = find_jdk(repo)
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="lexicon-java-") as temporary:
        work = Path(temporary)
        classes = work / "classes"
        classes.mkdir()
        sources = sorted((adapter / "compiler" / "src").glob("*.java"))
        if not sources:
            raise FileNotFoundError("Java compiler helper sources are missing")
        run(
            [
                str(jdk / "bin" / executable("javac")),
                "-encoding",
                "UTF-8",
                "-d",
                str(classes),
                *[str(source) for source in sources],
            ],
            adapter,
        )
        compiler_dir = output / "compiler"
        compiler_dir.mkdir(exist_ok=True)
        run(
            [
                str(jdk / "bin" / executable("jar")),
                "--create",
                "--file",
                str(compiler_dir / "lexicon-java-compiler.jar"),
                "-C",
                str(classes),
                ".",
            ],
            adapter,
        )
        run(
            [
                str(jdk / "bin" / executable("jlink")),
                "--add-modules",
                "java.base,jdk.compiler",
                "--output",
                str(output / "runtime"),
                "--strip-debug",
                "--no-header-files",
                "--no-man-pages",
                "--compress=2",
            ],
            adapter,
        )
    run(
        [
            "go",
            "build",
            "-trimpath",
            "-buildvcs=false",
            "-o",
            str(output / executable("lexicon-java")),
            ".",
        ],
        adapter,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    if args.output.exists():
        shutil.rmtree(args.output)
    build_java_adapter(args.repo.resolve(), args.output.resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

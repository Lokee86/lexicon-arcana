#!/usr/bin/env python3
"""Build and verify the private C-family Clang/LibTooling frontend."""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "lexicon" / "adapters" / "c-family-clang"


def executable_name(name: str) -> str:
    return name + ".exe" if os.name == "nt" else name


def helper_version() -> str:
    version = (SOURCE / "VERSION").read_text(encoding="utf-8").strip()
    if not version:
        raise RuntimeError("C-family Clang helper VERSION is empty")
    return version


def cmake() -> str:
    found = shutil.which("cmake")
    if not found:
        raise FileNotFoundError("cmake executable not found on PATH")
    return found


def ctest() -> str:
    found = shutil.which("ctest")
    if not found:
        raise FileNotFoundError("ctest executable not found on PATH")
    return found


def run(command: list[str], cwd: Path) -> None:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=cwd, check=True)


def build(
    output: Path,
    llvm_dir: Path | None,
    clang_dir: Path | None,
    config: str,
) -> Path:
    output = output.resolve()
    build_dir = output / "cmake"
    build_dir.mkdir(parents=True, exist_ok=True)

    configure = [
        cmake(),
        "-S",
        str(SOURCE),
        "-B",
        str(build_dir),
        f"-DCMAKE_BUILD_TYPE={config}",
    ]
    if llvm_dir is not None:
        configure.append(f"-DLLVM_DIR={llvm_dir.resolve()}")
    if clang_dir is not None:
        configure.append(f"-DClang_DIR={clang_dir.resolve()}")
    run(configure, ROOT)
    run([cmake(), "--build", str(build_dir), "--config", config], ROOT)
    run(
        [
            ctest(),
            "--test-dir",
            str(build_dir),
            "--output-on-failure",
            "-C",
            config,
        ],
        ROOT,
    )

    candidates = [
        build_dir / executable_name("lexicon-c-family-clang"),
        build_dir / config / executable_name("lexicon-c-family-clang"),
    ]
    built = next((candidate for candidate in candidates if candidate.is_file()), None)
    if built is None:
        raise FileNotFoundError("C-family Clang helper build output was not found")

    destination = output / executable_name("lexicon-c-family-clang")
    shutil.copy2(built, destination)
    verify(destination)
    return destination


def verify(helper: Path) -> None:
    expected = f"lexicon-c-family-clang {helper_version()}"
    completed = subprocess.run(
        [helper, "--version"],
        cwd=helper.parent,
        check=True,
        capture_output=True,
        text=True,
    )
    actual = completed.stdout.strip()
    if actual != expected:
        raise RuntimeError(f"{helper} reported {actual!r}; expected {expected!r}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--llvm-dir", type=Path)
    parser.add_argument("--clang-dir", type=Path)
    parser.add_argument("--config", default="Release")
    args = parser.parse_args()

    helper = build(args.output, args.llvm_dir, args.clang_dir, args.config)
    print(helper)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

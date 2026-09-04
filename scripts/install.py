#!/usr/bin/env python3
"""Install a combined Lexicon + Arcana bundle produced by scripts/workflow.py."""

from __future__ import annotations

import argparse
import platform
import shutil
from pathlib import Path


def executable_name(name: str) -> str:
    return name + ".exe" if platform.system().lower() == "windows" else name


def copy_file(source: Path, destination: Path) -> None:
    if not source.is_file():
        raise FileNotFoundError(source)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def resolve_components(components: list[str]) -> list[str]:
    selected = list(dict.fromkeys(components))
    if not selected:
        return ["lexicon", "arcana"]
    allowed = {"lexicon", "arcana"}
    if any(component not in allowed for component in selected):
        raise ValueError("components must contain only lexicon or arcana")
    return selected


def install(source: Path, bin_dir: Path, components: list[str]) -> None:
    source = source.resolve()
    bin_dir = bin_dir.resolve()
    source_bin = source / "bin"
    selected = resolve_components(components)
    names = [executable_name(name) for name in selected]
    for name in names:
        if not (source_bin / name).is_file():
            raise FileNotFoundError(f"combined bundle is missing {source_bin / name}")

    bin_dir.mkdir(parents=True, exist_ok=True)
    for name in names:
        copy_file(source_bin / name, bin_dir / name)

    if "lexicon" in selected:
        adapters = source / "adapters"
        if not adapters.is_dir():
            raise FileNotFoundError(f"combined bundle is missing {adapters}")
        destination = bin_dir / "adapters"
        if destination.exists():
            shutil.rmtree(destination)
        shutil.copytree(adapters, destination)

    print(f"installed {', '.join(selected)} to {bin_dir}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        type=Path,
        default=Path(__file__).resolve().parent,
        help="combined build or extracted bundle; defaults to this bundle",
    )
    parser.add_argument("--bin-dir", type=Path, required=True, help="directory receiving the commands")
    parser.add_argument(
        "--component",
        action="append",
        choices=("lexicon", "arcana"),
        dest="components",
        help="component to install; repeatable; defaults to Lexicon + Arcana",
    )
    args = parser.parse_args()
    try:
        install(args.source, args.bin_dir, args.components or [])
    except (OSError, ValueError) as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

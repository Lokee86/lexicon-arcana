#!/usr/bin/env python3
"""Install a combined Grimoire bundle produced by scripts/workflow.py."""

from __future__ import annotations

import argparse
import platform
import shutil
from pathlib import Path


def executable_name(name: str) -> str:
    return name + ".exe" if platform.system().lower() == "windows" else name


def native_library_name() -> str:
    system = platform.system().lower()
    if system == "windows":
        return "lodestone_ffi.dll"
    if system == "darwin":
        return "liblodestone_ffi.dylib"
    return "liblodestone_ffi.so"


def default_skill_roots() -> tuple[Path, ...]:
    return (
        Path.home() / ".agents" / "skills",
        Path.home() / ".hermes" / "skills",
    )


def copy_file(source: Path, destination: Path) -> None:
    if not source.is_file():
        raise FileNotFoundError(source)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def resolve_components(components: list[str]) -> list[str]:
    selected = list(dict.fromkeys(components))
    if not selected:
        return ["grimoire", "lexicon", "arcana"]
    if "grimoire" in selected:
        for dependency in ("lexicon", "arcana"):
            if dependency not in selected:
                selected.append(dependency)
    return selected


def install(
    source: Path,
    bin_dir: Path,
    components: list[str],
    skill_roots: list[Path] | tuple[Path, ...] | None = None,
) -> None:
    source = source.resolve()
    bin_dir = bin_dir.resolve()
    source_bin = source / "bin"
    source_native = source / "native"
    selected = resolve_components(components)
    names = [executable_name(name) for name in selected]
    for name in names:
        if not (source_bin / name).is_file():
            raise FileNotFoundError(f"combined bundle is missing {source_bin / name}")
    library = source_native / native_library_name()
    skill = source / "skills" / "grimoire" / "SKILL.md"
    if "grimoire" in selected and not library.is_file():
        raise FileNotFoundError(f"combined bundle is missing {library}")
    if "grimoire" in selected and not skill.is_file():
        raise FileNotFoundError(f"combined bundle is missing {skill}")
    bin_dir.mkdir(parents=True, exist_ok=True)
    for name in names:
        copy_file(source_bin / name, bin_dir / name)
    if "grimoire" in selected:
        copy_file(library, bin_dir / library.name)
        for skills_dir in default_skill_roots() if skill_roots is None else skill_roots:
            copy_file(skill, Path(skills_dir) / "grimoire" / "SKILL.md")
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
    parser.add_argument("--source", type=Path, default=Path(__file__).resolve().parent, help="combined build or extracted bundle; defaults to this bundle")
    parser.add_argument("--bin-dir", type=Path, required=True, help="directory receiving the commands")
    parser.add_argument("--component", action="append", choices=("grimoire", "lexicon", "arcana"), dest="components", help="component to install; repeatable; defaults to all")
    parser.add_argument("--skills-dir", action="append", type=Path, dest="skills_dirs", help="agent skills root receiving grimoire/SKILL.md; repeatable; defaults to ~/.agents/skills and ~/.hermes/skills")
    parser.add_argument("--skip-skills", action="store_true", help="install binaries without installing the Grimoire agent skill")
    args = parser.parse_args()
    try:
        skill_roots = () if args.skip_skills else args.skills_dirs
        install(args.source, args.bin_dir, args.components or [], skill_roots)
    except OSError as error:
        parser.error(str(error))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

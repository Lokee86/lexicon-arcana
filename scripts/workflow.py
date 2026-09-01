#!/usr/bin/env python3
"""Root build, test, install, and release workflow for the Grimoire monorepo."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path
from typing import Iterable, Sequence

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_LODESTONE_ROOT = ROOT.parent / "lodestone"
LODESTONE_COMMIT = "372abb7b9d5c9fb19eeaf92774849505e1dfbade"
LODESTONE_GO_VERSION = "v0.0.0-20260727052216-372abb7b9d5c"
PITLORD_VERSION = "v0.1.2"
DEFAULT_BUILD = ROOT / "build"
DEFAULT_DIST = ROOT / "dist"
LEGAL_FILES = ("LICENSE.md", "LICENSING.md", "THIRD_PARTY_NOTICES.md")
LODESTONE_LICENSE = Path("licenses") / "lodestone-Apache-2.0.txt"
VERSION_PATTERN = re.compile(r"^[0-9A-Za-z][0-9A-Za-z.+_-]*$")
ARCANA_PROTOCOL = "arcana.query.v1"
ARCANA_PROTOCOL_VERSION = 1
ARCANA_REQUIRED_OPERATIONS = {
    "stats", "export_graph", "search_nodes", "neighbors", "impact", "paths", "operational_role"
}
LEXICON_TOOLS = ROOT / "lexicon" / "tools"
if str(LEXICON_TOOLS) not in sys.path:
    sys.path.insert(0, str(LEXICON_TOOLS))
from java_release import build_java_adapter
from package_release import build_csharp


def executable_name(name: str, platform_name: str | None = None) -> str:
    return name + ".exe" if (platform_name or platform.system()).lower() == "windows" else name


def native_library_name(platform_name: str | None = None) -> str:
    name = (platform_name or platform.system()).lower()
    if name == "windows":
        return "lodestone_ffi.dll"
    if name == "darwin":
        return "liblodestone_ffi.dylib"
    return "liblodestone_ffi.so"


def pitlord_command() -> str:
    configured = os.environ.get("PITLORD")
    if configured:
        return configured
    found = shutil.which("pitlord")
    if found:
        return found
    sibling = ROOT.parent / "pitlord" / "bin" / executable_name("pitlord")
    if sibling.is_file():
        return str(sibling)
    raise FileNotFoundError(
        f"Pitlord is required; install github.com/Lokee86/pitlord/cmd/pitlord@{PITLORD_VERSION} "
        "or set PITLORD"
    )


def default_skill_roots() -> tuple[Path, ...]:
    return (
        Path.home() / ".agents" / "skills",
        Path.home() / ".hermes" / "skills",
    )


def lodestone_root() -> Path:
    configured = os.environ.get("LODESTONE_ROOT")
    return Path(configured).resolve() if configured else DEFAULT_LODESTONE_ROOT.resolve()


def verify_lodestone_checkout() -> Path:
    """Require the exact Lodestone source identity consumed by this release."""
    root = lodestone_root()
    binding_module = root / "bindings" / "go" / "go.mod"
    if not binding_module.is_file():
        raise FileNotFoundError(
            f"Lodestone Go bindings were not found at {binding_module}; set LODESTONE_ROOT"
        )
    module_text = binding_module.read_text(encoding="utf-8")
    if "module github.com/Lokee86/lodestone/bindings/go" not in module_text:
        raise RuntimeError(f"unexpected Lodestone Go module identity in {binding_module}")
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=root, check=True, capture_output=True, text=True
    )
    actual = completed.stdout.strip()
    if actual != LODESTONE_COMMIT:
        raise RuntimeError(
            f"Lodestone checkout {actual or '<unknown>'} does not match pinned {LODESTONE_COMMIT}"
        )
    root_module = (ROOT / "go.mod").read_text(encoding="utf-8")
    requirement = f"github.com/Lokee86/lodestone/bindings/go {LODESTONE_GO_VERSION}"
    if requirement not in root_module:
        raise RuntimeError(f"go.mod does not require pinned Lodestone version {LODESTONE_GO_VERSION}")
    return root


def target_label(platform_name: str | None = None, machine: str | None = None) -> str:
    system = (platform_name or platform.system()).lower()
    machine_name = (machine or platform.machine()).lower().replace(" ", "-")
    aliases = {"amd64": "x86_64", "x64": "x86_64", "arm64": "aarch64"}
    return f"{system}-{aliases.get(machine_name, machine_name)}"


def validate_version(version: str) -> str:
    if not VERSION_PATTERN.fullmatch(version):
        raise ValueError("version must contain only letters, numbers, '.', '+', '_', or '-'")
    return version


def validate_jobs(jobs: int) -> int:
    if jobs < 1:
        raise ValueError("jobs must be at least 1")
    return jobs


def bounded_env(jobs: int) -> dict[str, str]:
    jobs = validate_jobs(jobs)
    environment = os.environ.copy()
    environment["GOMAXPROCS"] = str(jobs)
    environment["CARGO_BUILD_JOBS"] = str(jobs)
    environment["RUST_TEST_THREADS"] = str(jobs)
    return environment


def run(command: Sequence[str], cwd: Path, env: dict[str, str] | None = None) -> None:
    print("+", " ".join(str(part) for part in command))
    subprocess.run(list(command), cwd=cwd, env=env, check=True)


def cargo_command() -> str:
    found = shutil.which("cargo")
    if found:
        return found
    candidate = Path.home() / ".cargo" / "bin" / executable_name("cargo")
    if candidate.is_file():
        return str(candidate)
    raise FileNotFoundError("cargo executable not found on PATH or in ~/.cargo/bin")


def copy_file(source: Path, destination: Path) -> None:
    if not source.is_file():
        raise FileNotFoundError(f"expected build output was not found: {source}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def write_utf8(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content.encode("utf-8"))


def build(
    version: str,
    output: Path,
    jobs: int = 1,
    components: Sequence[str] = ("grimoire", "lexicon", "arcana"),
) -> Path:
    """Build selected owning components into one disposable, CPU-bounded layout."""
    validate_version(version)
    jobs = validate_jobs(jobs)
    selected = resolve_install_components(components)
    build_env = bounded_env(jobs)
    output = output.resolve()
    if output == ROOT:
        raise ValueError("build output must not replace the source tree")
    if output.exists():
        shutil.rmtree(output)
    bin_dir = output / "bin"
    bin_dir.mkdir(parents=True)
    for name in LEGAL_FILES:
        copy_file(ROOT / name, output / name)

    cargo = cargo_command() if "arcana" in selected or "grimoire" in selected else ""
    release_env = build_env.copy()
    release_env["GRIMOIRE_RELEASE_VERSION"] = version
    lodestone = None
    if "grimoire" in selected:
        copy_file(ROOT / LODESTONE_LICENSE, output / LODESTONE_LICENSE)
        lodestone = verify_lodestone_checkout()
        go_ldflags = f"-X github.com/Lokee86/grimoire/internal/app.Version={version}"
        run(
            ["go", "build", "-p", str(jobs), "-trimpath", "-buildvcs=false", "-ldflags", go_ldflags,
             "-o", str(bin_dir / executable_name("grimoire")), "./cmd/grimoire"],
            ROOT, build_env,
        )

    if "lexicon" in selected:
        lexicon_ldflags = f"-X github.com/Lokee86/lexicon/internal/cli.version={version}"
        run(
            ["go", "build", "-p", str(jobs), "-trimpath", "-buildvcs=false", "-ldflags", lexicon_ldflags,
             "-o", str(bin_dir / executable_name("lexicon")), "./cmd/lexicon"],
            ROOT / "lexicon", build_env,
        )
        package_lexicon_adapters(output, cargo_command(), jobs, build_env)

    if "arcana" in selected:
        run(
            [cargo, "build", "--jobs", str(jobs), "--release", "--locked", "--manifest-path", str(ROOT / "arcana" / "Cargo.toml")],
            ROOT, release_env,
        )
        copy_file(ROOT / "arcana" / "target" / "release" / executable_name("arcana"), bin_dir / executable_name("arcana"))
        verify_arcana_protocol(output)

    if "grimoire" in selected:
        assert lodestone is not None
        native_dir = output / "native"
        native_dir.mkdir(parents=True)
        lodestone_manifest = lodestone / "Cargo.toml"
        if not lodestone_manifest.is_file():
            raise FileNotFoundError(
                f"Lodestone repository was not found at {lodestone}; set LODESTONE_ROOT"
            )
        run(
            [cargo, "build", "--jobs", str(jobs), "--release", "--locked", "--manifest-path", str(lodestone_manifest), "-p", "lodestone-ffi"],
            lodestone, release_env,
        )
        copy_file(lodestone / "target" / "release" / native_library_name(), native_dir / native_library_name())
        copy_file(ROOT / "skills" / "grimoire" / "SKILL.md", output / "skills" / "grimoire" / "SKILL.md")

    verify_versions(output, version, selected)
    return output


def package_lexicon_adapters(
    output: Path,
    cargo: str,
    jobs: int,
    environment: dict[str, str],
) -> None:
    """Prepare runtime adapter resources without requiring a source checkout."""
    source = ROOT / "lexicon" / "adapters"
    destination = output / "adapters"
    shutil.copytree(
        source,
        destination,
        ignore=shutil.ignore_patterns(
            "__pycache__", "*.pyc", ".pytest_cache", ".tools", "bin", "obj",
            "target", "node_modules", "dist", "runtime.facts.jsonl"
        ),
    )
    for language in ("c-family", "go", "gdscript", "kotlin", "generic"):
        run(
            [
                "go", "build", "-p", str(jobs), "-trimpath", "-buildvcs=false",
                "-o", str(destination / language / executable_name(f"lexicon-{language}")), ".",
            ],
            source / language,
            environment,
        )
    build_java_adapter(ROOT / "lexicon", destination / "java")
    build_csharp(ROOT / "lexicon", destination / "csharp")
    rust_manifest = source / "rust" / "Cargo.toml"
    if rust_manifest.is_file():
        run(
            [
                cargo, "build", "--jobs", str(jobs), "--release", "--locked",
                "--manifest-path", str(rust_manifest),
            ],
            ROOT,
            environment,
        )
        copy_file(
            source / "rust" / "target" / "release" / executable_name("lexicon-rust-adapter"),
            destination / "rust" / executable_name("lexicon-rust"),
        )

    typescript = destination / "typescript"
    if (typescript / "package-lock.json").is_file():
        npm = shutil.which("npm") or shutil.which("npm.cmd")
        if not npm:
            raise FileNotFoundError("npm executable not found on PATH")
        run([npm, "ci", "--silent"], typescript, environment)
        run([npm, "run", "build", "--silent"], typescript, environment)


def verify_versions(build_root: Path, version: str, components: Sequence[str] = ("grimoire", "lexicon", "arcana")) -> None:
    """Exercise version commands for every selected build component."""
    expected = {
        "grimoire": ("version", version),
        "lexicon": ("version", f"lexicon version {version}"),
        "arcana": ("--version", f"Arcana {version}"),
    }
    commands = [
        ([build_root / "bin" / executable_name(name), argument], value)
        for name in components
        for argument, value in [expected[name]]
    ]
    for command, expected_value in commands:
        completed = subprocess.run(command, cwd=build_root, check=True, capture_output=True, text=True)
        actual = completed.stdout.strip()
        if actual != expected_value:
            raise RuntimeError(f"{command[0]} reported {actual!r}; expected {expected_value!r}")


def verify_arcana_protocol(build_root: Path) -> None:
    """Require the built Arcana binary to satisfy the integration protocol contract."""
    arcana = build_root / "bin" / executable_name("arcana")
    with tempfile.TemporaryDirectory(prefix="grimoire-arcana-protocol-") as temporary:
        root = Path(temporary)
        facts = root / "facts.tsv"
        snapshot = root / "snapshot"
        write_utf8(facts, "version\t4\n")
        subprocess.run(
            [arcana, "import-facts", "--facts", str(facts), "--output", str(snapshot)],
            cwd=build_root,
            check=True,
            capture_output=True,
            text=True,
        )
        request = json.dumps({"id": "build-capabilities", "op": "capabilities"}) + "\n"
        completed = subprocess.run(
            [arcana, "protocol", "--snapshot", str(snapshot)],
            cwd=build_root,
            input=request,
            check=True,
            capture_output=True,
            text=True,
        )
    try:
        response = json.loads(completed.stdout.strip())
    except json.JSONDecodeError as error:
        raise RuntimeError("Arcana capabilities response is malformed") from error
    if response.get("ok") is not True:
        message = response.get("error", {}).get("message", "unknown protocol error")
        raise RuntimeError(f"Arcana capabilities request failed: {message}")
    try:
        result = response["result"]
        protocol = result["protocol"]
        version = result["version"]
        operations = set(result["operations"])
    except (KeyError, TypeError) as error:
        raise RuntimeError("Arcana capabilities response is malformed") from error
    if protocol != ARCANA_PROTOCOL or version != ARCANA_PROTOCOL_VERSION:
        raise RuntimeError(
            f"Arcana protocol {protocol!r} version {version!r} is incompatible with "
            f"{ARCANA_PROTOCOL!r} version {ARCANA_PROTOCOL_VERSION}"
        )
    missing = sorted(ARCANA_REQUIRED_OPERATIONS - operations)
    if missing:
        raise RuntimeError("Arcana protocol is missing required operations: " + ", ".join(missing))


def test(jobs: int = 1) -> None:
    """Run policy, documentation, and component suites with bounded parallelism."""
    jobs = validate_jobs(jobs)
    environment = bounded_env(jobs)
    cargo = cargo_command()
    pitlord = pitlord_command()
    verify_lodestone_checkout()
    policy = "tools/pitlord/policy.json"
    run([pitlord, "validate", "--policy", policy], ROOT, environment)
    run([pitlord, "check", "--repo", ".", "--policy", policy, "--timeout", "2m"], ROOT, environment)
    run([sys.executable, "scripts/check_docs.py"], ROOT, environment)
    go_test = ["go", "test", "-p", str(jobs), "-parallel", str(jobs), "./..."]
    run(go_test, ROOT, environment)
    run(go_test, ROOT / "lexicon", environment)
    for adapter in ("java", "kotlin"):
        run(go_test, ROOT / "lexicon" / "adapters" / adapter, environment)
    run(
        [sys.executable, "lexicon/adapters/csharp/tests/test_adapter.py"],
        ROOT,
        environment,
    )
    run([
        cargo, "test", "--jobs", str(jobs), "--all-targets", "--locked",
        "--manifest-path", str(ROOT / "arcana" / "Cargo.toml"),
        "--", "--test-threads", str(jobs),
    ], ROOT, environment)


def resolve_install_components(components: Sequence[str]) -> list[str]:
    selected = list(dict.fromkeys(components))
    allowed = {"grimoire", "lexicon", "arcana"}
    if not selected or any(component not in allowed for component in selected):
        raise ValueError("components must contain one or more of: grimoire, lexicon, arcana")
    if "grimoire" in selected:
        for dependency in ("lexicon", "arcana"):
            if dependency not in selected:
                selected.append(dependency)
    return selected


def install(
    source: Path,
    bin_dir: Path,
    components: Sequence[str] = ("grimoire", "lexicon", "arcana"),
    skill_roots: Sequence[Path] | None = None,
) -> None:
    """Install selected components and Grimoire's shared agent skill."""
    source = source.resolve()
    bin_dir = bin_dir.resolve()
    source_bin = source / "bin"
    source_native = source / "native"
    selected = resolve_install_components(components)
    required = [executable_name(name) for name in selected]
    for name in required:
        if not (source_bin / name).is_file():
            raise FileNotFoundError(f"combined build is missing {source_bin / name}")
    library = source_native / native_library_name()
    skill = source / "skills" / "grimoire" / "SKILL.md"
    if "grimoire" in selected and not library.is_file():
        raise FileNotFoundError(f"combined build is missing {library}")
    if "grimoire" in selected and not skill.is_file():
        raise FileNotFoundError(f"combined build is missing {skill}")

    bin_dir.mkdir(parents=True, exist_ok=True)
    for name in required:
        copy_file(source_bin / name, bin_dir / name)
    if "grimoire" in selected:
        # Keep the native library beside Grimoire so existing discovery works
        # without setting GRIMOIRE_VECTOR_ENGINE.
        copy_file(library, bin_dir / library.name)
        for skills_dir in default_skill_roots() if skill_roots is None else skill_roots:
            copy_file(skill, Path(skills_dir) / "grimoire" / "SKILL.md")
    if "lexicon" in selected:
        adapters = source / "adapters"
        if not adapters.is_dir():
            raise FileNotFoundError(f"combined build is missing {adapters}")
        destination = bin_dir / "adapters"
        if destination.exists():
            shutil.rmtree(destination)
        shutil.copytree(adapters, destination)
    print(f"installed {', '.join(selected)} to {bin_dir}")


def _fixed_zip(source: Path, archive: Path) -> None:
    archive.parent.mkdir(parents=True, exist_ok=True)
    executable_names = {
        "grimoire", "grimoire.exe", "lexicon", "lexicon.exe",
        "arcana", "arcana.exe", "install.py",
    }
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as output:
        for path in sorted(source.rglob("*")):
            if not path.is_file():
                continue
            relative = path.relative_to(source).as_posix()
            info = zipfile.ZipInfo(relative, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            executable = (
                path.name in executable_names
                or path.suffix.lower() == ".exe"
                or bool(path.stat().st_mode & 0o111)
            )
            mode = 0o100755 if executable else 0o100644
            info.external_attr = mode << 16
            output.writestr(info, path.read_bytes())


def _stage_files(stage: Path, files: Iterable[tuple[Path, str]], version: str) -> None:
    stage.mkdir(parents=True, exist_ok=True)
    for source, relative in files:
        copy_file(source, stage / relative)
    write_utf8(stage / "VERSION", version + "\n")


def package_artifacts(build_root: Path, output: Path, version: str, platform_name: str | None = None, machine: str | None = None) -> Path:
    """Create independent component archives, a combined archive, and SHA-256 sums."""
    validate_version(version)
    build_root = build_root.resolve()
    output = output.resolve()
    release_root = output / version
    if release_root.exists():
        shutil.rmtree(release_root)
    release_root.mkdir(parents=True)
    target = target_label(platform_name, machine)
    exe = lambda name: executable_name(name, platform_name)
    library = native_library_name(platform_name)
    archives: list[Path] = []

    with tempfile.TemporaryDirectory(prefix="grimoire-release-") as temporary:
        staging = Path(temporary)
        common_legal = [(ROOT / name, name) for name in LEGAL_FILES]
        lodestone_legal = (ROOT / LODESTONE_LICENSE, LODESTONE_LICENSE.as_posix())
        specs = {
            "grimoire": [
                (build_root / "bin" / exe("grimoire"), exe("grimoire")),
                (build_root / "native" / library, library),
                (build_root / "skills" / "grimoire" / "SKILL.md", "skills/grimoire/SKILL.md"),
                *common_legal,
                lodestone_legal,
            ],
            "lexicon": [(build_root / "bin" / exe("lexicon"), exe("lexicon")), *common_legal],
            "arcana": [(build_root / "bin" / exe("arcana"), exe("arcana")), *common_legal],
        }
        for component, files in specs.items():
            component_stage = staging / component
            _stage_files(component_stage, files, version)
            if component == "lexicon":
                shutil.copytree(build_root / "adapters", component_stage / "adapters")
            archive = release_root / f"{component}-{version}-{target}.zip"
            _fixed_zip(component_stage, archive)
            archives.append(archive)

        combined_stage = staging / "combined"
        _stage_files(combined_stage, [*common_legal, lodestone_legal], version)
        for relative in ("bin", "native", "adapters", "skills"):
            shutil.copytree(build_root / relative, combined_stage / relative)
        copy_file(ROOT / "scripts" / "install.py", combined_stage / "install.py")
        combined_archive = release_root / f"grimoire-bundle-{version}-{target}.zip"
        _fixed_zip(combined_stage, combined_archive)
        archives.append(combined_archive)

    manifest = {
        "version": version,
        "target": target,
        "artifacts": [archive.name for archive in archives],
        "combined_layout": {
            "executables": "bin/",
            "lodestone_library": "native/",
            "lexicon_adapters": "adapters/",
            "agent_skills": "skills/",
            "installer": "install.py",
        },
    }
    write_utf8(release_root / "release-manifest.json", json.dumps(manifest, indent=2) + "\n")
    checksum_lines = []
    for archive in sorted(archives):
        checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
        checksum_lines.append(f"{checksum}  {archive.name}")
    write_utf8(release_root / "SHA256SUMS.txt", "\n".join(checksum_lines) + "\n")
    return release_root


def release(version: str, output: Path, jobs: int = 1) -> Path:
    validate_version(version)
    jobs = validate_jobs(jobs)
    output = output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="grimoire-release-build-") as temporary:
        build_root = build(version, Path(temporary) / "build", jobs)
        return package_artifacts(build_root, output, version)


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    build_parser = subparsers.add_parser("build", help="build selected components into one disposable layout")
    build_parser.add_argument("--version", default="dev")
    build_parser.add_argument("--output", type=Path, default=DEFAULT_BUILD)
    build_parser.add_argument("--jobs", type=int, default=1, help="maximum build workers; defaults to 1")
    build_parser.add_argument("--component", action="append", choices=("grimoire", "lexicon", "arcana"), dest="components", help="component to build; repeatable; selecting grimoire includes Lexicon and Arcana")

    test_parser = subparsers.add_parser("test", help="run all component-owned test suites")
    test_parser.add_argument("--jobs", type=int, default=1, help="maximum package and test workers; defaults to 1")

    install_parser = subparsers.add_parser("install", help="install a combined build into a selected bin directory")
    install_parser.add_argument("--source", type=Path, default=DEFAULT_BUILD)
    install_parser.add_argument("--bin-dir", type=Path, required=True)
    install_parser.add_argument("--component", action="append", choices=("grimoire", "lexicon", "arcana"), dest="components", help="component to install; repeatable; defaults to all")
    install_parser.add_argument("--skills-dir", action="append", type=Path, dest="skills_dirs", help="agent skills root receiving grimoire/SKILL.md; repeatable; defaults to ~/.agents/skills and ~/.hermes/skills")
    install_parser.add_argument("--skip-skills", action="store_true", help="install binaries without installing the Grimoire agent skill")

    release_parser = subparsers.add_parser("release", help="test, build, package, and checksum a release")
    release_parser.add_argument("--version", required=True)
    release_parser.add_argument("--output", type=Path, default=DEFAULT_DIST)
    release_parser.add_argument("--jobs", type=int, default=1, help="maximum build and test workers; defaults to 1")

    subparsers.add_parser("smoke", help="run deterministic workflow packaging and install smoke checks")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    try:
        if args.command == "build":
            build(args.version, args.output, args.jobs, args.components or ("grimoire", "lexicon", "arcana"))
        elif args.command == "test":
            test(args.jobs)
        elif args.command == "install":
            skill_roots = () if args.skip_skills else args.skills_dirs
            install(
                args.source,
                args.bin_dir,
                args.components or ("grimoire", "lexicon", "arcana"),
                skill_roots,
            )
        elif args.command == "release":
            test(args.jobs)
            release(args.version, args.output, args.jobs)
        elif args.command == "smoke":
            from test_workflow import run_smoke
            run_smoke()
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as error:
        print(f"workflow: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

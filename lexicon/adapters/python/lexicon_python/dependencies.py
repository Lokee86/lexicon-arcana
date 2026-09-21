"""Deterministic Python manifest and repository-local dependency facts."""

from __future__ import annotations

import re
import tomllib
from pathlib import Path
from typing import Any

from .bindings import BindingResolver, resolve_relative_module
from .model import Facts, RepositorySnapshot

_DEPENDENCY = re.compile(r"^([A-Za-z0-9][A-Za-z0-9._-]*)(?:\s*(.*))?$")


def _attributes(category: str, source: str, constraint: str = "", *, path: bool = False) -> dict[str, Any]:
    return {
        "build": category == "build",
        "category": category,
        "constraint": constraint,
        "dev": category in {"development", "test"},
        "optional": category == "optional",
        "path": path,
        "peer": category == "peer",
        "source": source,
    }


def _target(facts: Facts, name: str, *, local_path: str = "") -> str:
    normalized = name.replace("\\", "/")
    identity = f"dependency:python:{normalized}"
    display_path = local_path or f"@dependencies/python/{normalized}"
    return facts.add_node(
        "module",
        normalized,
        display_path,
        identity,
        identity=identity,
        attributes={"dependency": True, "ecosystem": "python"},
    )


def _literal_requirement(value: object) -> tuple[str, str] | None:
    if not isinstance(value, str) or not value or value.startswith(("-", "git+", "http:", "https:")):
        return None
    match = _DEPENDENCY.match(value.strip())
    if not match:
        return None
    constraint = (match.group(2) or "").strip()
    if constraint.count("[") != constraint.count("]"):
        return None
    return match.group(1), constraint


def _add_manifest_dependency(facts: Facts, repository_id: str, name: str, constraint: str, source: str, category: str, *, local_path: str = "") -> None:
    facts.add_edge(
        repository_id,
        _target(facts, name if not local_path else local_path, local_path=local_path),
        "depends-on",
        attributes=_attributes(category, source, constraint, path=bool(local_path)),
    )


def _read_project_dependencies(root: Path) -> tuple[list[tuple[str, str, str]], bool]:
    filename = root / "pyproject.toml"
    if not filename.is_file():
        return [], False
    try:
        data = tomllib.loads(filename.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError):
        return [], False
    project = data.get("project")
    if not isinstance(project, dict):
        return [], False
    result: list[tuple[str, str, str]] = []
    dependencies = project.get("dependencies")
    if isinstance(dependencies, list):
        for item in dependencies:
            parsed = _literal_requirement(item)
            if parsed:
                result.append((*parsed, "pyproject.toml:project.dependencies"))
    optional = project.get("optional-dependencies")
    if isinstance(optional, dict):
        for extra in sorted(optional):
            values = optional[extra]
            if not isinstance(values, list):
                continue
            for item in values:
                parsed = _literal_requirement(item)
                if parsed:
                    result.append((*parsed, f"pyproject.toml:project.optional-dependencies.{extra}"))
    return result, bool(dependencies is not None)


def _read_requirements(root: Path) -> list[tuple[str, str, str]]:
    result: list[tuple[str, str, str]] = []
    for filename in sorted(root.glob("requirements*.txt")):
        try:
            lines = filename.read_text(encoding="utf-8").splitlines()
        except OSError:
            continue
        for line in lines:
            value = line.split("#", 1)[0].strip()
            if not value or value.startswith(("-r", "--", "-c", "-f", "git+", "http:", "https:")):
                continue
            local_path = ""
            if value.startswith(("-e ", "--editable ")):
                value = value.split(None, 1)[1].strip() if " " in value else ""
                local_path = value.replace("\\", "/")
                if local_path.startswith("/") or local_path == ".." or local_path.startswith("../"):
                    local_path = ""
                else:
                    local_path = local_path.removeprefix("./")
                if local_path:
                    result.append((local_path, "", f"{filename.name}:editable"))
                continue
            parsed = _literal_requirement(value)
            if parsed:
                result.append((*parsed, f"{filename.name}:requirement"))
    return result


def add_dependency_facts(facts: Facts, snapshot: RepositorySnapshot) -> None:
    repository_id = next(identifier for identifier, record in facts.nodes.items() if record.kind == "repository")
    manifest, had_project_dependencies = _read_project_dependencies(snapshot.root)
    if not had_project_dependencies:
        manifest.extend(_read_requirements(snapshot.root))
    for name, constraint, source in manifest:
        local_path = name if source.endswith(":editable") else ""
        _add_manifest_dependency(
            facts,
            repository_id,
            name,
            constraint,
            source,
            "test" if source.endswith(".test") or "requirements-test" in source else "optional" if "optional-dependencies" in source else "development" if "requirements-dev" in source else "runtime",
            local_path=local_path,
        )

    bindings = BindingResolver(facts)
    for info in facts.imports:
        requested = resolve_relative_module(info)
        module_name = bindings.resolve_module_name(requested, info.module_name) if requested else None
        if not module_name or module_name not in facts.modules:
            continue
        source_module = facts.modules.get(info.module_name)
        if source_module:
            facts.add_edge(
                source_module,
                facts.modules[module_name],
                "depends-on",
                attributes=_attributes("local", requested, path=True),
            )

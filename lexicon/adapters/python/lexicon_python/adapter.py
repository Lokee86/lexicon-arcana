"""Orchestrate extraction, repository-wide resolution, and emission."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from . import __version__
from .dependencies import add_dependency_facts
from .emission import emit_records, write_records
from .parallel import extract_repository
from .resolution import resolve_facts
from .semantic_outcomes import emit_outcome_facts


def build_facts(
    repo: Path,
    changed_files: list[str] | None = None,
    removed_files: list[str] | None = None,
    workers: int = 1,
    shards: int = 1,
    merge_fan_in: int = 2,
) -> list[dict[str, Any]]:
    snapshot, facts = extract_repository(repo, workers, shards, merge_fan_in)
    resolve_facts(facts, snapshot.contexts)
    emit_outcome_facts(facts, snapshot.contexts)
    add_dependency_facts(facts, snapshot)
    return emit_records(facts, __version__, changed_files, removed_files)


def write_facts(
    repo: Path,
    output: Path,
    changed_files: list[str] | None = None,
    removed_files: list[str] | None = None,
    workers: int = 1,
    shards: int = 1,
    merge_fan_in: int = 2,
) -> None:
    write_records(
        build_facts(repo, changed_files, removed_files, workers, shards, merge_fan_in),
        output,
    )

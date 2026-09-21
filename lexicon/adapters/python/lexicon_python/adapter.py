"""Orchestrate extraction, repository-wide resolution, and emission."""

from __future__ import annotations

from pathlib import Path
from typing import Any

from . import __version__
from .contract import clear_source_cache
from .dependencies import add_dependency_facts
from .emission import emit_records, iter_records, write_records
from .model import Facts
from .parallel import extract_repository
from .resolution import resolve_facts
from .semantic_outcomes import emit_outcome_facts


def _analyze(
    repo: Path,
    workers: int,
    shards: int,
    merge_fan_in: int,
) -> Facts:
    snapshot, facts = extract_repository(repo, workers, shards, merge_fan_in)
    resolve_facts(facts)
    emit_outcome_facts(facts)
    add_dependency_facts(facts, snapshot)

    # Nodes/edges/unresolved are now complete. Drop AST-bearing resolution
    # indexes and source representations before canonical emission/sorting.
    facts.release_analysis_state()
    clear_source_cache()
    return facts


def build_facts(
    repo: Path,
    changed_files: list[str] | None = None,
    removed_files: list[str] | None = None,
    workers: int = 1,
    shards: int = 1,
    merge_fan_in: int = 2,
) -> list[dict[str, Any]]:
    facts = _analyze(repo, workers, shards, merge_fan_in)
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
    facts = _analyze(repo, workers, shards, merge_fan_in)
    write_records(
        iter_records(facts, __version__, changed_files, removed_files),
        output,
    )

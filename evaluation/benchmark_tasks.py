from __future__ import annotations

import json
from pathlib import Path
from typing import Any

SUPPORTED_CATEGORIES = {
    "architectural-exploration",
    "unclear-ownership",
    "cross-language-change",
    "impact-analysis",
    "source-plus-rationale",
}


def load_task_suite(path: Path, workspace_root: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if payload.get("version") != 2:
        raise ValueError("agent benchmark task suite must use version 2")
    sections = payload.get("evidence_sections")
    if sections != ["evidence"]:
        raise ValueError("version 2 tasks use one generic evidence section")
    tasks = payload.get("tasks")
    if not isinstance(tasks, list) or not tasks:
        raise ValueError("task suite must contain tasks")

    seen: set[str] = set()
    categories: set[str] = set()
    for task in tasks:
        task_id = str(task.get("id", "")).strip()
        category = str(task.get("category", "")).strip()
        prompt = str(task.get("prompt", "")).strip()
        if not task_id or task_id in seen:
            raise ValueError(f"task id must be unique: {task_id!r}")
        if category not in SUPPORTED_CATEGORIES:
            raise ValueError(f"task {task_id!r} has unsupported category {category!r}")
        if not prompt or "BENCHMARK_EVIDENCE_JSON" in prompt or "Deliver:" in prompt:
            raise ValueError(f"task {task_id!r} exposes benchmark answer structure")
        rubric = task.get("rubric") or {}
        if not rubric.get("dimensions") or not rubric.get("required_evidence"):
            raise ValueError(f"task {task_id!r} needs a hidden rubric")
        repository = workspace_root / str(task.get("repository", ""))
        if not repository.is_dir():
            raise ValueError(f"task {task_id!r} repository is unavailable: {repository}")
        if not bool(task.get("retired")):
            validate_evidence_prefixes(task, repository)
        task["repo"] = repository
        seen.add(task_id)
        categories.add(category)
    missing = SUPPORTED_CATEGORIES - categories
    if missing:
        raise ValueError(f"task suite is missing categories: {', '.join(sorted(missing))}")
    return payload


def validate_evidence_prefixes(task: dict[str, Any], repository: Path) -> None:
    task_id = str(task.get("id", ""))
    rubric = task.get("rubric") or {}
    for evidence in rubric.get("required_evidence") or []:
        prefix = str(evidence.get("path_prefix", "")).strip().rstrip("/")
        candidate = repository / prefix
        prefix_matches = candidate.exists() or any(candidate.parent.glob(candidate.name + "*"))
        if not prefix or not prefix_matches:
            raise ValueError(f"task {task_id!r} has unavailable evidence prefix: {prefix!r}")

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shutil
import time

from benchmark_provenance import assert_frozen, git_changes, git_commit, verify_build_version
from benchmark_runner import BenchmarkEnvironment, collect_runs, prepare_worktree
from benchmark_tasks import load_task_suite, validate_evidence_prefixes

ROOT = Path(r"C:\!bin\workspace")
GRIMOIRE_REPO = ROOT / "grimoire"
DEFAULT_TASKS = GRIMOIRE_REPO / "evaluation" / "agent_benchmark_tasks.v2.json"
DEFAULT_BUILD = Path(os.environ.get("GRIMOIRE_BENCH_BUILD", GRIMOIRE_REPO / "build"))
DEFAULT_OUTPUT = GRIMOIRE_REPO / "evaluation" / "results" / "agent-benchmark-v2"
DEFAULT_CHECKOUTS = ROOT / "benchmark-checkouts" / "agent-benchmark-v2"
MODEL = "gpt-5.6-sol"
PROVIDER = "openai-codex"
DEFAULT_CONDITIONS = ("plain", "cbm", "grimoire")
CONDITIONS = (*DEFAULT_CONDITIONS, "lexicon-arcana")
IGNORED_HARNESS_CHANGES = (
    ".warlock/",
    ".worktrees/",
    "build/",
    "evaluation/__pycache__/",
    "evaluation/results/",
)


def relevant_harness_changes(changes: list[str]) -> list[str]:
    relevant = []
    for change in changes:
        path = change[3:].replace("\\", "/") if len(change) > 3 else change
        if not any(path.startswith(prefix) for prefix in IGNORED_HARNESS_CHANGES):
            relevant.append(change)
    return relevant


def select_tasks(suite: dict, selected: list[str]) -> list[dict]:
    tasks = suite["tasks"]
    if not selected:
        return tasks
    wanted = set(selected)
    chosen = [task for task in tasks if task["id"] in wanted]
    missing = wanted - {task["id"] for task in chosen}
    if missing:
        raise ValueError(f"unknown task ids: {', '.join(sorted(missing))}")
    return chosen


def initialize_summary(
    output: Path,
    *,
    task_suite: Path,
    model: str,
    provider: str,
    conditions: tuple[str, ...],
    provenance: dict,
) -> dict:
    started_at = datetime.now(timezone.utc).isoformat()
    summary_path = output / "summary.json"
    if not summary_path.is_file():
        return {
            "schema": "grimoire.agent-benchmark.v2",
            "task_suite": str(task_suite),
            "model": model,
            "provider": provider,
            "conditions": list(conditions),
            "parallel_within_task": False,
            "condition_execution": "sequential",
            "sequential_tasks": True,
            "provenance": provenance,
            "started_at": started_at,
            "last_run_started_at": started_at,
            "tasks": {},
        }
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    expected = {
        "schema": "grimoire.agent-benchmark.v2",
        "task_suite": str(task_suite),
        "model": model,
        "provider": provider,
        "provenance": provenance,
    }
    for key, value in expected.items():
        if summary.get(key) != value:
            raise ValueError(
                f"existing benchmark summary has incompatible {key}: "
                f"{summary.get(key)!r} != {value!r}"
            )
    summary.setdefault("tasks", {})
    summary["conditions"] = list(dict.fromkeys([*summary.get("conditions", []), *conditions]))
    summary["parallel_within_task"] = False
    summary["condition_execution"] = "sequential"
    summary["sequential_tasks"] = True
    summary["last_run_started_at"] = started_at
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description="Run the version 2 grounded agent benchmark suite.")
    parser.add_argument("--tasks", type=Path, default=DEFAULT_TASKS)
    parser.add_argument("--build", type=Path, default=DEFAULT_BUILD)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--checkout-root", type=Path, default=DEFAULT_CHECKOUTS)
    parser.add_argument("--task", action="append", default=[])
    parser.add_argument("--condition", action="append", choices=CONDITIONS, default=[])
    parser.add_argument("--model", default=MODEL)
    parser.add_argument("--provider", default=PROVIDER)
    parser.add_argument("--check", action="store_true", help="validate tasks and dependencies without running benchmarks")
    args = parser.parse_args()
    args.output = args.output.resolve()
    args.checkout_root = args.checkout_root.resolve()

    task_suite = args.tasks.resolve()
    suite = load_task_suite(task_suite, ROOT)
    tasks = select_tasks(suite, args.task)
    conditions = tuple(dict.fromkeys(args.condition or DEFAULT_CONDITIONS))
    environment = BenchmarkEnvironment(ROOT, args.build.resolve(), args.model, args.provider)
    harness_commit = git_commit(GRIMOIRE_REPO)
    build_version = f"benchmark-{harness_commit[:12]}"
    changes = relevant_harness_changes(git_changes(GRIMOIRE_REPO))

    if args.check:
        report = {
            "ready": False,
            "tasks": [task["id"] for task in tasks],
            "conditions": conditions,
            "build": str(args.build.resolve()),
            "harness_commit": harness_commit,
            "expected_build_version": build_version,
            "harness_changes": changes,
        }
        try:
            environment.require_dependencies(conditions)
            provenance = environment.provenance(task_suite, conditions)
            verify_build_version(provenance, build_version)
            report["provenance"] = provenance
            report["ready"] = not changes
        except (OSError, RuntimeError, ValueError) as error:
            report["error"] = str(error)
        print(json.dumps(report, indent=2))
        return 0 if report["ready"] else 1

    if changes:
        raise RuntimeError(
            "benchmark harness has uncommitted source changes; commit or isolate them before running:\n"
            + "\n".join(changes)
        )
    environment.rebuild(build_version, conditions)
    environment.require_dependencies(conditions)
    provenance = environment.provenance(task_suite, conditions)
    verify_build_version(provenance, build_version)
    args.output.mkdir(parents=True, exist_ok=True)
    args.checkout_root.mkdir(parents=True, exist_ok=True)

    summary = initialize_summary(
        args.output,
        task_suite=task_suite,
        model=args.model,
        provider=args.provider,
        conditions=conditions,
        provenance=provenance,
    )
    for task in tasks:
        task_output = args.output / task["id"]
        task_output.mkdir(parents=True, exist_ok=True)
        checkouts: dict[str, Path] = {}
        commits: set[str] = set()
        for condition in conditions:
            checkout = args.checkout_root / task["id"] / condition
            commits.add(prepare_worktree(task["repo"], checkout, task["revision"]))
            validate_evidence_prefixes(task, checkout)
            checkouts[condition] = checkout
        if len(commits) != 1:
            raise RuntimeError(f"conditions resolved different commits: {sorted(commits)}")
        commit = next(iter(commits))

        preparation: dict[str, object] = {}
        cbm_project = None
        cbm_cache = task_output / "cbm-cache"
        if "cbm" in conditions:
            preparation["cbm"] = environment.prewarm_cbm(checkouts["cbm"], cbm_cache, task_output)
            cbm_project = preparation["cbm"]["project"]
        if "grimoire" in conditions:
            preparation["grimoire"] = environment.prewarm_grimoire(checkouts["grimoire"], task_output)
        condition_context: dict[str, dict[str, str]] = {}
        if "lexicon-arcana" in conditions:
            component_summary, component_context = environment.prewarm_lexicon_arcana(
                checkouts["lexicon-arcana"], task_output
            )
            preparation["lexicon-arcana"] = component_summary
            condition_context["lexicon-arcana"] = component_context

        profiles: dict[str, str] = {}
        profile_provenance: dict[str, dict] = {}
        for condition in conditions:
            profile = f"bench-v2-{task['id'][:20]}-{condition}"
            audit = task_output / "grimoire.mcp-audit.jsonl" if condition == "grimoire" else None
            if audit and audit.exists():
                audit.unlink()
            environment.prepare_profile(
                profile,
                condition,
                checkouts[condition],
                cbm_cache=cbm_cache if condition == "cbm" else None,
                audit_log=audit,
            )
            profiles[condition] = profile
            profile_provenance[condition] = environment.profile_identity(profile)

        runs: dict[str, dict] = {}
        required_prefixes = [
            item["path_prefix"] for item in task["rubric"]["required_evidence"]
        ]
        for condition in conditions:
            assert_frozen(environment.provenance(task_suite, conditions), provenance)
            active = {
                condition: environment.launch(
                    task,
                    condition,
                    checkouts[condition],
                    task_output,
                    profiles[condition],
                    cbm_project,
                    condition_context.get(condition),
                )
            }
            runs.update(collect_runs(
                active,
                checkout_by_condition={condition: checkouts[condition]},
                output=task_output,
                expected_sections=suite["evidence_sections"],
                required_path_prefixes=required_prefixes,
            ))
            assert_frozen(environment.provenance(task_suite, conditions), provenance)
        summary["tasks"][task["id"]] = {
            "category": task["category"],
            "repository": str(task["repo"]),
            "commit": commit,
            "rubric": task["rubric"],
            "preparation": preparation,
            "profiles": profiles,
            "profile_provenance": profile_provenance,
            "runs": runs,
        }
        (args.output / "summary.partial.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
        for context in condition_context.values():
            for key in ("export_dir", "bin_dir"):
                path = context.get(key)
                if path:
                    shutil.rmtree(path, ignore_errors=True)
        time.sleep(3)

    summary["completed_at"] = datetime.now(timezone.utc).isoformat()
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    partial = args.output / "summary.partial.json"
    partial.unlink(missing_ok=True)
    for task in tasks:
        shutil.rmtree(args.output / task["id"] / "cbm-cache", ignore_errors=True)
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

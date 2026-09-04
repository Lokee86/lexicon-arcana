from __future__ import annotations

from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

EVALUATION = Path(__file__).resolve().parents[2]
if str(EVALUATION) not in sys.path:
    sys.path.insert(0, str(EVALUATION))

from benchmark_grounding import validate_answer
from benchmark_process import isolated_path, prepare_worktree
from benchmark_provenance import git_changes, git_commit, verify_build_version
from benchmark_runner import BenchmarkEnvironment, COMMON_PROMPT
from benchmark_tasks import load_task_suite, validate_evidence_prefixes
from run_agent_benchmark import relevant_harness_changes

ROOT = Path(r"C:\!bin\workspace")
REPO = ROOT / "grimoire"
TASK_SUITE = REPO / "evaluation" / "agent_benchmark_tasks.v2.json"
OUTPUT = Path(__file__).resolve().parent
CHECKOUT_ROOT = ROOT / "benchmark-checkouts" / "agent-benchmark-v2-lexicon-arcana-codex"
BUILD = REPO / "build"
MODEL = "gpt-5.6-sol"
TASK_IDS = (
    "grimoire-state-maintenance-ownership",
    "space-rocks-room-scale-architecture",
    "detekt-cli-gradle-plugin-divergence",
    "space-rocks-distant-player-locator",
)
ORIGINAL_CODEX_SHA256 = "39e9e041ea33ac34aad9578adfe660c5c7a6dc8f82620b77623960f9352a6ef3"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def find_codex() -> Path:
    candidates = sorted(
        (Path.home() / ".vscode" / "extensions").glob(
            "openai.chatgpt-*/bin/windows-x86_64/codex.exe"
        ),
        reverse=True,
    )
    if not candidates:
        raise RuntimeError("no VS Code bundled Codex CLI found")
    return candidates[0]


def parse_events(path: Path) -> tuple[dict | None, dict[str, int], int]:
    usage = None
    counts: Counter[str] = Counter()
    valid_lines = 0
    if not path.is_file():
        return usage, {}, valid_lines
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        valid_lines += 1
        event_type = str(event.get("type") or "unknown")
        counts[event_type] += 1
        if event_type == "turn.completed" and isinstance(event.get("usage"), dict):
            usage = event["usage"]
    return usage, dict(counts), valid_lines


def codex_version(codex: Path) -> str:
    result = subprocess.run(
        [str(codex), "--version"], text=True, capture_output=True, check=True, timeout=30
    )
    return result.stdout.strip() or result.stderr.strip()


def run_task(
    *,
    codex: Path,
    environment: BenchmarkEnvironment,
    task: dict,
    checkout: Path,
    task_output: Path,
    component_context: dict[str, str],
    skill_text: str,
    evidence_sections: list[str],
) -> dict:
    answer = task_output / "lexicon-arcana.stdout.txt"
    events = task_output / "lexicon-arcana.events.jsonl"
    stderr = task_output / "lexicon-arcana.stderr.txt"
    prompt = (
        COMMON_PROMPT
        + "\nBenchmark condition constraints:\n"
        + "This is the Lexicon + Arcana component-only condition. Do not read or use any "
        + "Grimoire or CBM skill, CLI, MCP server, executable, or prepared state. The only optional "
        + "discovery surface for this condition is the prepared Lexicon export and Arcana protocol "
        + "described in the frozen condition skill below. Normal direct shell/Git/file inspection "
        + "remains allowed. Use only the local checked-out repository and prepared local component "
        + "state. Do not use GitHub, network, web, browser, app, remote-repository, or cloud tools.\n\n"
        + "<LEXICON_ARCANA_CONDITION_SKILL>\n"
        + skill_text
        + "\n</LEXICON_ARCANA_CONDITION_SKILL>\n"
        + "\nTask:\n"
        + task["prompt"]
        + "\n"
    )
    env = os.environ.copy()
    env["LEXICON_BENCH_EXPORT"] = component_context["export_dir"]
    env["ARCANA_BENCH_SNAPSHOT"] = component_context["arcana_snapshot"]
    env["LEXICON_BENCH_REPO"] = str(checkout)
    env["PATH"] = isolated_path(
        Path(component_context["bin_dir"]),
        [environment.cbm_binary.parent, environment.grimoire_build / "bin"],
    )
    command = [
        str(codex),
        "exec",
        "--json",
        "--ignore-user-config",
        "--ignore-rules",
        "--enable",
        "fast_mode",
        "--disable",
        "apps",
        "--disable",
        "plugins",
        "--disable",
        "browser_use",
        "--disable",
        "in_app_browser",
        "--disable",
        "computer_use",
        "--disable",
        "skill_search",
        "-c",
        'model_reasoning_effort="high"',
        "-c",
        "shell_environment_policy.inherit=all",
        "-m",
        MODEL,
        "-s",
        "danger-full-access",
        "-C",
        str(checkout),
        "-o",
        str(answer),
        prompt,
    ]
    started = time.perf_counter()
    exit_code = 0
    timed_out = False
    with events.open("w", encoding="utf-8", newline="") as out, stderr.open(
        "w", encoding="utf-8", newline=""
    ) as err:
        try:
            result = subprocess.run(
                command,
                cwd=checkout,
                env=env,
                text=True,
                stdout=out,
                stderr=err,
                timeout=2400,
            )
            exit_code = result.returncode
        except subprocess.TimeoutExpired:
            exit_code = -9
            timed_out = True
    elapsed = time.perf_counter() - started
    usage, event_counts, event_lines = parse_events(events)
    required_prefixes = [
        item["path_prefix"] for item in task["rubric"]["required_evidence"]
    ]
    grounding = validate_answer(
        checkout,
        answer.read_text(encoding="utf-8", errors="replace") if answer.is_file() else "",
        exit_code=exit_code,
        expected_sections=evidence_sections,
        audit_log=None,
        require_grimoire_handles=False,
        required_path_prefixes=required_prefixes,
    )
    (task_output / "lexicon-arcana.grounding.json").write_text(
        json.dumps(grounding.to_dict(), indent=2) + "\n", encoding="utf-8"
    )
    return {
        "exit_code": exit_code,
        "timed_out": timed_out,
        "elapsed_seconds": round(elapsed, 3),
        "usage": usage,
        "event_counts": event_counts,
        "event_lines": event_lines,
        "answer_bytes": answer.stat().st_size if answer.is_file() else 0,
        "grounding": grounding.to_dict(),
        "execution_valid": exit_code == 0,
        "grounding_valid": grounding.valid,
        "eligible_for_scoring": exit_code == 0 and grounding.valid,
        "quality_assessed": False,
        "completed_at": datetime.now(timezone.utc).isoformat(),
    }


def main() -> int:
    changes = relevant_harness_changes(git_changes(REPO))
    if changes:
        raise RuntimeError("benchmark source is dirty:\n" + "\n".join(changes))
    harness_commit = git_commit(REPO)
    build_version = f"benchmark-{harness_commit[:12]}"
    codex = find_codex()
    environment = BenchmarkEnvironment(ROOT, BUILD, MODEL, "openai-codex")
    environment.rebuild(build_version, ("lexicon-arcana",))
    environment.require_dependencies(("lexicon-arcana",))
    component_provenance = environment.provenance(TASK_SUITE, ("lexicon-arcana",))
    verify_build_version(component_provenance, build_version)

    suite = load_task_suite(TASK_SUITE, ROOT)
    by_id = {task["id"]: task for task in suite["tasks"]}
    tasks = [by_id[task_id] for task_id in TASK_IDS]
    skill_path = REPO / "evaluation" / "skills" / "lexicon-arcana" / "SKILL.md"
    skill_text = skill_path.read_text(encoding="utf-8")
    OUTPUT.mkdir(parents=True, exist_ok=True)
    CHECKOUT_ROOT.mkdir(parents=True, exist_ok=True)
    summary = {
        "schema": "grimoire.agent-benchmark.v2.codex-component-ablation",
        "runner": "Codex CLI exec",
        "model": MODEL,
        "reasoning_effort": "high",
        "fast_mode": True,
        "sandbox": "danger-full-access (benchmark prompt remains read-only)",
        "local_tool_isolation": "apps/plugins/browser/computer-use/skill-search disabled",
        "condition": "lexicon-arcana",
        "tasks": {},
        "harness_commit": harness_commit,
        "expected_build_version": build_version,
        "component_provenance": component_provenance,
        "codex": {
            "path": str(codex),
            "sha256": sha256(codex),
            "version": codex_version(codex),
            "original_2026_07_29_sha256": ORIGINAL_CODEX_SHA256,
            "same_binary_as_original": sha256(codex) == ORIGINAL_CODEX_SHA256,
        },
        "condition_skill": {"path": str(skill_path), "sha256": sha256(skill_path)},
        "started_at": datetime.now(timezone.utc).isoformat(),
    }
    (OUTPUT / "summary.partial.json").write_text(
        json.dumps(summary, indent=2) + "\n", encoding="utf-8"
    )

    for task in tasks:
        task_output = OUTPUT / task["id"]
        task_output.mkdir(parents=True, exist_ok=True)
        checkout = CHECKOUT_ROOT / task["id"]
        commit = prepare_worktree(task["repo"], checkout, task["revision"])
        validate_evidence_prefixes(task, checkout)
        preparation, context = environment.prewarm_lexicon_arcana(checkout, task_output)
        run = run_task(
            codex=codex,
            environment=environment,
            task=task,
            checkout=checkout,
            task_output=task_output,
            component_context=context,
            skill_text=skill_text,
            evidence_sections=suite["evidence_sections"],
        )
        summary["tasks"][task["id"]] = {
            "category": task["category"],
            "repository": str(task["repo"]),
            "commit": commit,
            "rubric": task["rubric"],
            "preparation": {"lexicon-arcana": preparation},
            "runs": {"lexicon-arcana": run},
        }
        (OUTPUT / "summary.partial.json").write_text(
            json.dumps(summary, indent=2) + "\n", encoding="utf-8"
        )
        for key in ("export_dir", "bin_dir"):
            path = context.get(key)
            if path:
                shutil.rmtree(path, ignore_errors=True)
        time.sleep(3)

    summary["completed_at"] = datetime.now(timezone.utc).isoformat()
    (OUTPUT / "summary.json").write_text(
        json.dumps(summary, indent=2) + "\n", encoding="utf-8"
    )
    (OUTPUT / "summary.partial.json").unlink(missing_ok=True)
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

from __future__ import annotations

from datetime import datetime, timezone
import json
from pathlib import Path
import shutil
import sys

ROOT = Path(r"C:\!bin\workspace")
REPO = ROOT / "grimoire"
EVALUATION = REPO / "evaluation"
BASE_RUNNER_DIR = REPO / "evaluation" / "results" / "agent-benchmark-v2-lexicon-arcana-codex-sol-high-fast-2026-09-02"
for path in (EVALUATION, BASE_RUNNER_DIR):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

import run_codex_component as base
from benchmark_process import prepare_worktree
from benchmark_runner import BenchmarkEnvironment
from benchmark_tasks import load_task_suite, validate_evidence_prefixes

OUTPUT = Path(__file__).resolve().parent
CHECKOUT = ROOT / "benchmark-checkouts" / "agent-benchmark-v2-la-stopping-prompt" / "detekt-cli-gradle-plugin-divergence"
TASK_SUITE = EVALUATION / "agent_benchmark_tasks.v2.json"
SKILL = EVALUATION / "skills" / "lexicon-arcana" / "SKILL.md"
TASK_ID = "detekt-cli-gradle-plugin-divergence"


def main() -> int:
    suite = load_task_suite(TASK_SUITE, ROOT)
    task = next(t for t in suite["tasks"] if t["id"] == TASK_ID)
    commit = prepare_worktree(task["repo"], CHECKOUT, task["revision"])
    validate_evidence_prefixes(task, CHECKOUT)

    env = BenchmarkEnvironment(ROOT, REPO / "build", base.MODEL, "openai-codex")
    env.require_dependencies(("lexicon-arcana",))
    task_output = OUTPUT / TASK_ID
    task_output.mkdir(parents=True, exist_ok=True)
    preparation, context = env.prewarm_lexicon_arcana(CHECKOUT, task_output)
    run = base.run_task(
        codex=base.find_codex(),
        environment=env,
        task=task,
        checkout=CHECKOUT,
        task_output=task_output,
        component_context=context,
        skill_text=SKILL.read_text(encoding="utf-8"),
        evidence_sections=suite["evidence_sections"],
    )
    summary = {
        "schema": "grimoire.agent-benchmark.v2.prompt-variant",
        "condition": "lexicon-arcana",
        "variant": "bounded-evidence-expansion",
        "model": base.MODEL,
        "task": TASK_ID,
        "commit": commit,
        "skill_path": str(SKILL),
        "preparation": preparation,
        "run": run,
        "completed_at": datetime.now(timezone.utc).isoformat(),
    }
    (OUTPUT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    for key in ("export_dir", "bin_dir"):
        if context.get(key):
            shutil.rmtree(context[key], ignore_errors=True)
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

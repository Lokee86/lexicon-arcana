from __future__ import annotations

from datetime import datetime, timezone
import importlib.util
import json
from pathlib import Path
import sys

ROOT = Path(r"C:\!bin\workspace")
REPO = ROOT / "grimoire"
EVALUATION = REPO / "evaluation"
BASE_DIR = REPO / "evaluation" / "results" / "agent-benchmark-v2-current-codex-detekt-control-2026-09-02"
BASE_RUNNER = BASE_DIR / "run_control.py"
OUTPUT = Path(__file__).resolve().parent
CHECKOUT_ROOT = ROOT / "benchmark-checkouts" / "agent-benchmark-v2-plain-bounded-completion"
TASK_SUITE = EVALUATION / "agent_benchmark_tasks.v2.json"
TASK_ID = "detekt-cli-gradle-plugin-divergence"

if str(EVALUATION) not in sys.path:
    sys.path.insert(0, str(EVALUATION))

spec = importlib.util.spec_from_file_location("detekt_current_control", BASE_RUNNER)
if spec is None or spec.loader is None:
    raise RuntimeError(f"could not load {BASE_RUNNER}")
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)

from benchmark_tasks import load_task_suite, validate_evidence_prefixes

COMPLETION_POLICY = r'''

Completion policy for this run:
Investigate only until the requested diagnosis is supported. Once you can identify the likely owner, explain the causal divergence, define the smallest correct fix boundary, and provide a sufficient verification plan with source support, stop repository exploration and answer.
Do not broaden into related execution paths, Git history, documentation, alternative hypotheses, additional test plumbing, or extra verification solely to increase confidence in conclusions that are already supported. Investigate further only when you can name a specific unresolved fact that prevents one of the requested conclusions. Do not seek redundant confirmation of an already-supported conclusion.
'''


def main() -> int:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    task_output = OUTPUT / TASK_ID
    task_output.mkdir(parents=True, exist_ok=True)

    suite = load_task_suite(TASK_SUITE, ROOT)
    task = next(t for t in suite["tasks"] if t["id"] == TASK_ID)

    base.OUTPUT = OUTPUT
    base.CHECKOUT_ROOT = CHECKOUT_ROOT
    base.COMMON_PROMPT = base.COMMON_PROMPT + COMPLETION_POLICY

    checkout = base.make_blind_checkout("plain")
    validate_evidence_prefixes(task, checkout)
    codex = base.find_codex()

    summary = {
        "schema": "grimoire.agent-benchmark.v2.prompt-variant",
        "condition": "plain",
        "variant": "bounded-completion-surface",
        "model": base.MODEL,
        "reasoning_effort": "high",
        "fast_mode": True,
        "task": TASK_ID,
        "task_revision": base.REVISION,
        "codex": {
            "path": str(codex),
            "sha256": base.sha256(codex),
            "version": base.codex_version(codex),
        },
        "history_isolation": "pruned clone; one benchmark ref; no remotes; all visible commits reachable from frozen revision",
        "completion_policy": COMPLETION_POLICY.strip(),
        "started_at": datetime.now(timezone.utc).isoformat(),
    }

    summary["run"] = base.run_condition(
        condition="plain",
        codex=codex,
        checkout=checkout,
        task=task,
        skill_text="",
        bins={},
        evidence_sections=suite["evidence_sections"],
    )
    summary["completed_at"] = datetime.now(timezone.utc).isoformat()
    (OUTPUT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import shutil

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("control", HERE / "run_control.py")
assert SPEC and SPEC.loader
control = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(control)


def main() -> int:
    task_output = HERE / control.TASK_ID
    contaminated = task_output / "contaminated-global-skill"
    contaminated.mkdir(parents=True, exist_ok=True)
    for name in ("grimoire.events.jsonl", "grimoire.stderr.txt", "grimoire.stdout.txt", "grimoire.grounding.json", "grimoire.mcp-audit.jsonl"):
        src = task_output / name
        if src.exists():
            dst = contaminated / name
            dst.unlink(missing_ok=True)
            shutil.move(str(src), str(dst))

    suite = control.load_task_suite(control.TASK_SUITE, control.ROOT)
    task = next(t for t in suite["tasks"] if t["id"] == control.TASK_ID)
    checkout = control.make_blind_checkout("grimoire-clean")
    control.validate_evidence_prefixes(task, checkout)
    codex = control.find_codex()
    bins = control.select_july_binaries()
    skill_text = (control.JULY_WORKTREE / "skills" / "grimoire" / "SKILL.md").read_text(encoding="utf-8")

    partial = HERE / "summary.partial.json"
    if partial.is_file():
        summary = json.loads(partial.read_text(encoding="utf-8"))
    else:
        summary = json.loads((HERE / "summary.json").read_text(encoding="utf-8"))
    summary["host_skill_discovery"] = False
    summary["contaminated_grimoire_attempt_archived"] = str(contaminated)
    summary["preparation"]["grimoire"] = control.prewarm_grimoire(checkout, task_output, bins)
    summary["runs"]["grimoire"] = control.run_condition(
        condition="grimoire",
        codex=codex,
        checkout=checkout,
        task=task,
        skill_text=skill_text,
        bins=bins,
        evidence_sections=suite["evidence_sections"],
    )
    (HERE / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"preparation": summary["preparation"]["grimoire"], "run": summary["runs"]["grimoire"]}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

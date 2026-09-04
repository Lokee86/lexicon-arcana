from __future__ import annotations

from collections import Counter
from datetime import datetime, timezone
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
from benchmark_process import isolated_path
from benchmark_runner import BenchmarkEnvironment, COMMON_PROMPT
from benchmark_tasks import load_task_suite, validate_evidence_prefixes

ROOT = Path(r"C:\!bin\workspace")
REPO = ROOT / "grimoire"
SPACE_ROCKS = ROOT / "space-rocks"
TASK_SUITE = REPO / "evaluation" / "agent_benchmark_tasks.v2.json"
OUTPUT = Path(__file__).resolve().parent
BUILD = REPO / "build"
MODEL = "gpt-5.6-sol"
TASK_ID = "space-rocks-distant-player-locator"
REVISION = "460da4af05c44d1835401fa853f5fc6b718262c8"
BLIND_CHECKOUT = ROOT / "benchmark-checkouts" / "agent-benchmark-v2-lexicon-arcana-codex-blind2" / TASK_ID


def run(*args: str, cwd: Path | None = None) -> str:
    result = subprocess.run(args, cwd=cwd, text=True, capture_output=True, check=True, timeout=600)
    return result.stdout.strip()


def make_blind_checkout() -> None:
    shutil.rmtree(BLIND_CHECKOUT, ignore_errors=True)
    BLIND_CHECKOUT.parent.mkdir(parents=True, exist_ok=True)
    run("git", "clone", "--no-local", "--no-checkout", str(SPACE_ROCKS), str(BLIND_CHECKOUT))
    run("git", "checkout", "--detach", REVISION, cwd=BLIND_CHECKOUT)
    refs = run("git", "for-each-ref", "--format=%(refname)", cwd=BLIND_CHECKOUT).splitlines()
    for ref in refs:
        if ref:
            run("git", "update-ref", "-d", ref, cwd=BLIND_CHECKOUT)
    run("git", "update-ref", "refs/heads/benchmark", REVISION, cwd=BLIND_CHECKOUT)
    run("git", "checkout", "benchmark", cwd=BLIND_CHECKOUT)
    run("git", "remote", "remove", "origin", cwd=BLIND_CHECKOUT)
    run("git", "reflog", "expire", "--expire=now", "--all", cwd=BLIND_CHECKOUT)
    run("git", "gc", "--prune=now", cwd=BLIND_CHECKOUT)
    head = run("git", "rev-parse", "HEAD", cwd=BLIND_CHECKOUT)
    if head != REVISION:
        raise RuntimeError(f"blind checkout HEAD mismatch: {head}")
    refs_after = run("git", "for-each-ref", "--format=%(refname) %(objectname)", cwd=BLIND_CHECKOUT).splitlines()
    expected = f"refs/heads/benchmark {REVISION}"
    if refs_after != [expected]:
        raise RuntimeError(f"unexpected refs after pruning: {refs_after!r}")
    remotes = run("git", "remote", cwd=BLIND_CHECKOUT).splitlines()
    if remotes:
        raise RuntimeError(f"unexpected remotes after pruning: {remotes!r}")
    all_commits = set(run("git", "rev-list", "--all", cwd=BLIND_CHECKOUT).splitlines())
    head_commits = set(run("git", "rev-list", "HEAD", cwd=BLIND_CHECKOUT).splitlines())
    if all_commits != head_commits:
        raise RuntimeError("blind checkout exposes commits not reachable from HEAD")


def parse_events(path: Path) -> tuple[dict | None, dict[str, int], int]:
    usage = None
    counts: Counter[str] = Counter()
    valid_lines = 0
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines() if path.is_file() else []:
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


def find_codex() -> Path:
    candidates = sorted((Path.home() / ".vscode" / "extensions").glob("openai.chatgpt-*/bin/windows-x86_64/codex.exe"), reverse=True)
    if not candidates:
        raise RuntimeError("no VS Code bundled Codex CLI found")
    return candidates[0]


def main() -> int:
    contaminated = OUTPUT / TASK_ID
    audit = OUTPUT / f"{TASK_ID}-contaminated-future-history"
    if contaminated.exists() and not audit.exists():
        contaminated.rename(audit)
    elif contaminated.exists():
        shutil.rmtree(contaminated)

    make_blind_checkout()
    suite = load_task_suite(TASK_SUITE, ROOT)
    task = next(t for t in suite["tasks"] if t["id"] == TASK_ID)
    validate_evidence_prefixes(task, BLIND_CHECKOUT)

    environment = BenchmarkEnvironment(ROOT, BUILD, MODEL, "openai-codex")
    environment.require_dependencies(("lexicon-arcana",))
    task_output = OUTPUT / TASK_ID
    task_output.mkdir(parents=True, exist_ok=True)
    preparation, context = environment.prewarm_lexicon_arcana(BLIND_CHECKOUT, task_output)

    skill_path = REPO / "evaluation" / "skills" / "lexicon-arcana" / "SKILL.md"
    skill_text = skill_path.read_text(encoding="utf-8")
    prompt = (
        COMMON_PROMPT
        + "\nBenchmark condition constraints:\n"
        + "This is the Lexicon + Arcana component-only condition. Do not read or use any Grimoire or CBM skill, CLI, MCP server, executable, or prepared state. "
        + "The only optional discovery surface for this condition is the prepared Lexicon export and Arcana protocol described in the frozen condition skill below. "
        + "Normal direct shell/Git/file inspection remains allowed. Use only the local checked-out repository and prepared local component state. "
        + "Do not use GitHub, network, web, browser, app, remote-repository, or cloud tools. The checkout has intentionally been pruned so Git exposes only history reachable from the frozen benchmark revision; do not attempt to recover or access unreachable/future commits.\n\n"
        + "<LEXICON_ARCANA_CONDITION_SKILL>\n" + skill_text + "\n</LEXICON_ARCANA_CONDITION_SKILL>\n"
        + "\nTask:\n" + task["prompt"] + "\n"
    )

    answer = task_output / "lexicon-arcana.stdout.txt"
    events = task_output / "lexicon-arcana.events.jsonl"
    stderr = task_output / "lexicon-arcana.stderr.txt"
    env = os.environ.copy()
    env["LEXICON_BENCH_EXPORT"] = context["export_dir"]
    env["ARCANA_BENCH_SNAPSHOT"] = context["arcana_snapshot"]
    env["LEXICON_BENCH_REPO"] = str(BLIND_CHECKOUT)
    env["PATH"] = isolated_path(Path(context["bin_dir"]), [environment.cbm_binary.parent, environment.grimoire_build / "bin"])
    codex = find_codex()
    command = [
        str(codex), "exec", "--json", "--ignore-user-config", "--ignore-rules",
        "--enable", "fast_mode",
        "--disable", "apps", "--disable", "plugins", "--disable", "browser_use", "--disable", "in_app_browser",
        "--disable", "computer_use", "--disable", "skill_search",
        "-c", 'model_reasoning_effort="high"', "-c", "shell_environment_policy.inherit=all",
        "-m", MODEL, "-s", "danger-full-access", "-C", str(BLIND_CHECKOUT), "-o", str(answer), prompt,
    ]
    started = time.perf_counter()
    timed_out = False
    exit_code = 0
    with events.open("w", encoding="utf-8", newline="") as out, stderr.open("w", encoding="utf-8", newline="") as err:
        try:
            result = subprocess.run(command, cwd=BLIND_CHECKOUT, env=env, text=True, stdout=out, stderr=err, timeout=2400)
            exit_code = result.returncode
        except subprocess.TimeoutExpired:
            exit_code = -9
            timed_out = True
    elapsed = time.perf_counter() - started

    usage, event_counts, event_lines = parse_events(events)
    required_prefixes = [item["path_prefix"] for item in task["rubric"]["required_evidence"]]
    grounding = validate_answer(
        BLIND_CHECKOUT,
        answer.read_text(encoding="utf-8", errors="replace") if answer.is_file() else "",
        exit_code=exit_code,
        expected_sections=suite["evidence_sections"],
        audit_log=None,
        require_grimoire_handles=False,
        required_path_prefixes=required_prefixes,
    )
    (task_output / "lexicon-arcana.grounding.json").write_text(json.dumps(grounding.to_dict(), indent=2) + "\n", encoding="utf-8")
    result_record = {
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
        "history_isolation": {
            "mode": "standalone pruned clone",
            "only_ref": f"refs/heads/benchmark {REVISION}",
            "all_commits_reachable_from_head": True,
            "future_history_exposed": False,
        },
    }

    summary_path = OUTPUT / "summary.partial.json"
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    summary["tasks"][TASK_ID] = {
        "category": task["category"],
        "repository": str(SPACE_ROCKS),
        "commit": REVISION,
        "rubric": task["rubric"],
        "preparation": {"lexicon-arcana": preparation},
        "runs": {"lexicon-arcana": result_record},
    }
    summary["history_isolation_note"] = "Locator task was rerun in a standalone pruned clone after an earlier attempt inspected future Git history. Contaminated artifacts are retained in a sibling audit directory and excluded from this summary."
    summary["completed_at"] = datetime.now(timezone.utc).isoformat()
    (OUTPUT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    summary_path.unlink(missing_ok=True)

    for key in ("export_dir", "bin_dir"):
        path = context.get(key)
        if path:
            shutil.rmtree(path, ignore_errors=True)
    print(json.dumps(result_record, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

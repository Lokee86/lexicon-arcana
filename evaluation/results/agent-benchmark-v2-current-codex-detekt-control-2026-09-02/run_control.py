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
from benchmark_process import isolated_path
from benchmark_runner import COMMON_PROMPT
from benchmark_tasks import load_task_suite, validate_evidence_prefixes

ROOT = Path(r"C:\!bin\workspace")
REPO = ROOT / "grimoire"
SOURCE = ROOT / "corpus" / "kotlin-detekt"
TASK_SUITE = REPO / "evaluation" / "agent_benchmark_tasks.v2.json"
OUTPUT = Path(__file__).resolve().parent
CHECKOUT_ROOT = ROOT / "benchmark-checkouts" / "agent-benchmark-v2-current-codex-detekt-control"
JULY_WORKTREE = ROOT / "grimoire-july-control"
JULY_BUILD = ROOT / "benchmark-tools" / "grimoire-july-build"
MODEL = "gpt-5.6-sol"
TASK_ID = "detekt-cli-gradle-plugin-divergence"
REVISION = "f9e1d5cc239ab740ce499b1edb36b872012648e2"
JULY_HARNESS_COMMIT = "0375e5f6e541970e6ef509ac655961405c82e0f3"
ORIGINAL_CODEX_SHA256 = "39e9e041ea33ac34aad9578adfe660c5c7a6dc8f82620b77623960f9352a6ef3"
ORIGINAL_BIN_SHA = {
    "grimoire": "881715c69fe2fb6a144b9005883217d341ef24b2cf9a05d09b7466d21f3d31cf",
    "lexicon": "157e9710f43ac99f3c7884536f49045fc9923dd0b028e569c5c70e64282fb082",
    "arcana": "6796a7a42cedae899edc3655bb00643100ecf1236c80618799676c827bc5ac14",
}


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(*args: str, cwd: Path | None = None, env: dict[str, str] | None = None, timeout: int = 600) -> str:
    result = subprocess.run(args, cwd=cwd, env=env, text=True, capture_output=True, check=True, timeout=timeout)
    return result.stdout.strip()


def find_codex() -> Path:
    candidates = sorted((Path.home() / ".vscode" / "extensions").glob("openai.chatgpt-*/bin/windows-x86_64/codex.exe"), reverse=True)
    if not candidates:
        raise RuntimeError("no VS Code bundled Codex CLI found")
    return candidates[0]


def codex_version(codex: Path) -> str:
    return run(str(codex), "--version", timeout=30)


def make_blind_checkout(condition: str) -> Path:
    checkout = CHECKOUT_ROOT / condition
    shutil.rmtree(checkout, ignore_errors=True)
    checkout.parent.mkdir(parents=True, exist_ok=True)
    run("git", "clone", "--no-local", "--no-checkout", str(SOURCE), str(checkout))
    run("git", "checkout", "--detach", REVISION, cwd=checkout)
    refs = run("git", "for-each-ref", "--format=%(refname)", cwd=checkout).splitlines()
    for ref in refs:
        if ref:
            run("git", "update-ref", "-d", ref, cwd=checkout)
    run("git", "update-ref", "refs/heads/benchmark", REVISION, cwd=checkout)
    run("git", "checkout", "benchmark", cwd=checkout)
    remotes = run("git", "remote", cwd=checkout).splitlines()
    for remote in remotes:
        if remote:
            run("git", "remote", "remove", remote, cwd=checkout)
    run("git", "reflog", "expire", "--expire=now", "--all", cwd=checkout)
    run("git", "gc", "--prune=now", cwd=checkout, timeout=1200)
    if run("git", "rev-parse", "HEAD", cwd=checkout) != REVISION:
        raise RuntimeError(f"{condition}: HEAD mismatch")
    refs_after = run("git", "for-each-ref", "--format=%(refname) %(objectname)", cwd=checkout).splitlines()
    if refs_after != [f"refs/heads/benchmark {REVISION}"]:
        raise RuntimeError(f"{condition}: unexpected refs: {refs_after!r}")
    if run("git", "remote", cwd=checkout):
        raise RuntimeError(f"{condition}: remote survived pruning")
    if set(run("git", "rev-list", "--all", cwd=checkout).splitlines()) != set(run("git", "rev-list", "HEAD", cwd=checkout).splitlines()):
        raise RuntimeError(f"{condition}: future history visible")
    return checkout


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
        typ = str(event.get("type") or "unknown")
        counts[typ] += 1
        if typ == "turn.completed" and isinstance(event.get("usage"), dict):
            usage = event["usage"]
    return usage, dict(counts), valid_lines


def select_july_binaries() -> dict[str, Path]:
    candidates = {
        "grimoire": [REPO / "bin" / "grimoire.exe", JULY_BUILD / "bin" / "grimoire.exe"],
        "lexicon": [JULY_BUILD / "bin" / "lexicon.exe", REPO / "bin" / "lexicon.exe"],
        "arcana": [REPO / "bin" / "arcana.exe", JULY_BUILD / "bin" / "arcana.exe"],
    }
    chosen: dict[str, Path] = {}
    for name, paths in candidates.items():
        exact = next((p for p in paths if p.is_file() and sha256(p) == ORIGINAL_BIN_SHA[name]), None)
        if exact is not None:
            chosen[name] = exact
        elif name == "lexicon" and (JULY_BUILD / "bin" / "lexicon.exe").is_file():
            chosen[name] = JULY_BUILD / "bin" / "lexicon.exe"
        else:
            raise RuntimeError(f"no usable July {name} binary")
    return chosen


def prewarm_grimoire(checkout: Path, task_output: Path, bins: dict[str, Path]) -> dict:
    env = os.environ.copy()
    env["PATH"] = isolated_path(JULY_BUILD / "bin", [ROOT / "cbm-bin", REPO / "build" / "bin"])
    started = time.perf_counter()
    result = subprocess.run([
        str(bins["grimoire"]), "status", "--root", str(checkout), "--force",
        "--lexicon-command", str(bins["lexicon"]), "--arcana-command", str(bins["arcana"]),
    ], cwd=checkout, env=env, text=True, capture_output=True, timeout=3600)
    elapsed = time.perf_counter() - started
    (task_output / "grimoire-prewarm.stdout.txt").write_text(result.stdout, encoding="utf-8")
    (task_output / "grimoire-prewarm.stderr.txt").write_text(result.stderr, encoding="utf-8")
    if result.returncode != 0:
        raise RuntimeError(f"Grimoire prewarm failed: {result.stderr[-2000:]}")
    payload = json.loads(result.stdout)
    return {"elapsed_seconds": round(elapsed, 3), "timings": payload.get("timings") or {}, "actions": payload.get("actions") or []}


def toml_string(value: str) -> str:
    return json.dumps(value)


def toml_array(values: list[str]) -> str:
    return json.dumps(values)


def run_condition(*, condition: str, codex: Path, checkout: Path, task: dict, skill_text: str, bins: dict[str, Path], evidence_sections: list[str]) -> dict:
    task_output = OUTPUT / TASK_ID
    answer = task_output / f"{condition}.stdout.txt"
    events = task_output / f"{condition}.events.jsonl"
    stderr = task_output / f"{condition}.stderr.txt"
    audit = task_output / "grimoire.mcp-audit.jsonl" if condition == "grimoire" else None
    if audit is not None:
        audit.unlink(missing_ok=True)

    prompt = COMMON_PROMPT + "\nBenchmark condition constraints:\n"
    if condition == "plain":
        prompt += (
            "This is the Plain condition. No repository discovery/index MCP, skill, prepared state, Grimoire, Lexicon, Arcana, or CBM may be used. "
            "Use normal local shell/Git/file inspection only. "
        )
    else:
        prompt += (
            "This is the Grimoire condition. Grimoire is the only optional repository discovery surface. Use it according to the frozen July condition skill below, "
            "alongside normal local shell/Git/file inspection when cheaper. Do not use Lexicon, Arcana, or CBM directly.\n\n"
            "<GRIMOIRE_CONDITION_SKILL>\n" + skill_text + "\n</GRIMOIRE_CONDITION_SKILL>\n"
        )
    prompt += (
        "Use only this local checked-out repository and local condition tooling. Do not use GitHub, network, web, browser, apps, plugins, remote repositories, cloud tools, "
        "or attempt to recover unreachable/future Git objects. The checkout exposes only history reachable from the frozen benchmark revision.\n\n"
        "Task:\n" + task["prompt"] + "\n"
    )

    env = os.environ.copy()
    blocked = [ROOT / "cbm-bin", REPO / "build" / "bin", REPO / "bin", JULY_BUILD / "bin"]
    selected = JULY_BUILD / "bin" if condition == "grimoire" else None
    env["PATH"] = isolated_path(selected, blocked)

    command = [
        str(codex), "exec", "--json", "--ignore-user-config", "--ignore-rules",
        "--enable", "fast_mode", "--enable", "skip_host_skill_discovery",
        "--disable", "apps", "--disable", "plugins", "--disable", "browser_use", "--disable", "in_app_browser",
        "--disable", "computer_use", "--disable", "skill_search",
        "-c", 'model_reasoning_effort="high"', "-c", "shell_environment_policy.inherit=all",
    ]
    if condition == "grimoire":
        mcp_args = ["mcp", "--root", str(checkout), "--state-mode", "refresh-if-needed"]
        if audit is not None:
            mcp_args += ["--audit-log", str(audit)]
        command += [
            "-c", "mcp_servers.grimoire.command=" + toml_string(str(bins["grimoire"])),
            "-c", "mcp_servers.grimoire.args=" + toml_array(mcp_args),
            "-c", "mcp_servers.grimoire.enabled=true",
        ]
    command += ["-m", MODEL, "-s", "danger-full-access", "-C", str(checkout), "-o", str(answer), prompt]

    started = time.perf_counter()
    timed_out = False
    exit_code = 0
    with events.open("w", encoding="utf-8", newline="") as out, stderr.open("w", encoding="utf-8", newline="") as err:
        try:
            result = subprocess.run(command, cwd=checkout, env=env, text=True, stdout=out, stderr=err, timeout=2400)
            exit_code = result.returncode
        except subprocess.TimeoutExpired:
            exit_code = -9
            timed_out = True
    elapsed = time.perf_counter() - started
    usage, event_counts, event_lines = parse_events(events)
    required_prefixes = [item["path_prefix"] for item in task["rubric"]["required_evidence"]]
    grounding = validate_answer(
        checkout,
        answer.read_text(encoding="utf-8", errors="replace") if answer.is_file() else "",
        exit_code=exit_code,
        expected_sections=evidence_sections,
        audit_log=audit,
        require_grimoire_handles=False,
        required_path_prefixes=required_prefixes,
    )
    (task_output / f"{condition}.grounding.json").write_text(json.dumps(grounding.to_dict(), indent=2) + "\n", encoding="utf-8")
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
        "completed_at": datetime.now(timezone.utc).isoformat(),
    }


def main() -> int:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    task_output = OUTPUT / TASK_ID
    task_output.mkdir(parents=True, exist_ok=True)
    suite = load_task_suite(TASK_SUITE, ROOT)
    task = next(t for t in suite["tasks"] if t["id"] == TASK_ID)
    codex = find_codex()
    bins = select_july_binaries()
    skill_path = JULY_WORKTREE / "skills" / "grimoire" / "SKILL.md"
    skill_text = skill_path.read_text(encoding="utf-8")

    checkouts = {condition: make_blind_checkout(condition) for condition in ("plain", "grimoire")}
    for checkout in checkouts.values():
        validate_evidence_prefixes(task, checkout)

    summary = {
        "schema": "grimoire.agent-benchmark.v2.current-codex-runtime-control",
        "purpose": "Detekt Plain/Grimoire rerun holding frozen task revision and July Grimoire toolchain constant; current Codex runtime is the intended variable.",
        "model": MODEL,
        "reasoning_effort": "high",
        "fast_mode": True,
        "sandbox": "danger-full-access (benchmark prompt remains read-only)",
        "task": TASK_ID,
        "task_revision": REVISION,
        "july_harness_commit": JULY_HARNESS_COMMIT,
        "codex": {"path": str(codex), "sha256": sha256(codex), "version": codex_version(codex), "original_sha256": ORIGINAL_CODEX_SHA256, "same_binary_as_original": sha256(codex) == ORIGINAL_CODEX_SHA256},
        "tool_binaries": {name: {"path": str(path), "sha256": sha256(path), "original_sha256": ORIGINAL_BIN_SHA[name], "exact_original": sha256(path) == ORIGINAL_BIN_SHA[name]} for name, path in bins.items()},
        "history_isolation": "separate pruned clones; one benchmark ref; no remotes; all visible commits reachable from frozen revision",
        "started_at": datetime.now(timezone.utc).isoformat(),
        "preparation": {},
        "runs": {},
    }
    (OUTPUT / "summary.partial.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")

    summary["runs"]["plain"] = run_condition(condition="plain", codex=codex, checkout=checkouts["plain"], task=task, skill_text=skill_text, bins=bins, evidence_sections=suite["evidence_sections"])
    (OUTPUT / "summary.partial.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")

    summary["preparation"]["grimoire"] = prewarm_grimoire(checkouts["grimoire"], task_output, bins)
    summary["runs"]["grimoire"] = run_condition(condition="grimoire", codex=codex, checkout=checkouts["grimoire"], task=task, skill_text=skill_text, bins=bins, evidence_sections=suite["evidence_sections"])
    summary["completed_at"] = datetime.now(timezone.utc).isoformat()
    (OUTPUT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    (OUTPUT / "summary.partial.json").unlink(missing_ok=True)
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

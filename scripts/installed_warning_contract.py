"""Release-consumer warning compatibility checks for installed Grimoire bundles."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess

KNOWN_REASON = "unsupported-macro-expansion"
FUTURE_REASON = "future-unresolved-reason"


def run_warning_contract(installed: Path, temporary: Path, base_environment: dict[str, str]) -> dict:
    known_root = temporary / "warning-known"
    known_root.mkdir()
    (known_root / "main.c").write_text(
        "#define PASTE(name) invoke_##name()\n"
        "int run(void) { return PASTE(task); }\n",
        encoding="utf-8",
    )
    known_status = _run_status(installed, known_root, base_environment)
    _require_actions(known_status)
    _require_lexicon_reason(installed, known_root, base_environment, "c-family", KNOWN_REASON)
    known_arcana = _arcana_snapshot(known_root)
    _require_text(known_arcana / "unresolved.tsv", KNOWN_REASON)
    warning_path = known_arcana / "compatibility.warnings"
    known_warnings = list(known_status.get("warnings") or [])
    if any(KNOWN_REASON in warning for warning in known_warnings):
        raise RuntimeError(f"known unresolved reason reached Grimoire warnings: {known_warnings}")
    if warning_path.exists() and KNOWN_REASON in warning_path.read_text(encoding="utf-8"):
        raise RuntimeError(f"known unresolved reason degraded to compatibility warning: {KNOWN_REASON}")

    future_root = temporary / "warning-future"
    future_root.mkdir()
    (future_root / "future.py").write_text("def FutureThing():\n    return 1\n", encoding="utf-8")
    future_adapters = temporary / "future-adapters"
    _write_future_adapter(future_adapters)
    future_environment = base_environment.copy()
    future_environment["LEXICON_ADAPTERS"] = os.fspath(future_adapters)
    future_status = _run_status(installed, future_root, future_environment)
    _require_actions(future_status)
    _require_lexicon_reason(installed, future_root, future_environment, "python", FUTURE_REASON)
    future_arcana = _arcana_snapshot(future_root)
    _require_text(future_arcana / "unresolved.tsv", FUTURE_REASON)
    _require_text(future_arcana / "compatibility.warnings", FUTURE_REASON)

    arcana_warnings = list((future_status.get("arcana") or {}).get("warnings") or [])
    top_warnings = list(future_status.get("warnings") or [])
    if not any(FUTURE_REASON in warning for warning in arcana_warnings):
        raise RuntimeError(f"Arcana status omitted future unresolved reason: {arcana_warnings}")
    if not any("Arcana compatibility warning" in warning and FUTURE_REASON in warning for warning in top_warnings):
        raise RuntimeError(f"Grimoire status omitted promoted compatibility warning: {top_warnings}")

    return {
        "known_reason": KNOWN_REASON,
        "future_reason": FUTURE_REASON,
        "future_warning": next(warning for warning in top_warnings if FUTURE_REASON in warning),
    }


def _run_status(installed: Path, root: Path, environment: dict[str, str]) -> dict:
    binary = installed / "bin" / _executable_name("grimoire")
    completed = subprocess.run(
        [os.fspath(binary), "status", "--root", os.fspath(root), "--force"],
        cwd=root,
        env=environment,
        check=True,
        capture_output=True,
        text=True,
        timeout=120,
    )
    return json.loads(completed.stdout)


def _require_actions(status: dict) -> None:
    actions = {item.get("name"): item.get("status") for item in status.get("actions") or []}
    for name in ("refresh-lexicon", "synchronize-arcana", "prepare-grimoire"):
        if actions.get(name) != "completed":
            raise RuntimeError(f"warning-contract action {name} did not complete: {actions}")


def _require_lexicon_reason(
    installed: Path,
    root: Path,
    environment: dict[str, str],
    language: str,
    reason: str,
) -> None:
    destination = root.parent / f"{root.name}-{language}-lexicon-export"
    binary = installed / "bin" / _executable_name("lexicon")
    subprocess.run(
        [
            os.fspath(binary), "export", "--repo", os.fspath(root),
            "--output", os.fspath(destination), "--languages", language,
        ],
        cwd=root,
        env=environment,
        check=True,
        capture_output=True,
        text=True,
        timeout=60,
    )
    records = [json.loads(line) for line in (destination / f"{language}.jsonl").read_text(encoding="utf-8").splitlines()]
    if not any(record.get("record") == "unresolved" and record.get("reason") == reason for record in records):
        raise RuntimeError(f"Lexicon export omitted unresolved reason {reason}")


def _arcana_snapshot(root: Path) -> Path:
    state = root / ".arcana"
    current = (state / "CURRENT").read_text(encoding="utf-8").strip()
    return state / "snapshots" / current.removeprefix("sha256:")


def _require_text(path: Path, expected: str) -> None:
    text = path.read_text(encoding="utf-8")
    if expected not in text:
        raise RuntimeError(f"{path.name} omitted {expected}: {text}")


def _executable_name(name: str) -> str:
    return name + ".exe" if os.name == "nt" else name


def _write_future_adapter(root: Path) -> None:
    package = root / "python" / "lexicon_python"
    package.mkdir(parents=True)
    (package / "__init__.py").write_text("", encoding="utf-8")
    (package / "__main__.py").write_text(
        """import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--repo")
parser.add_argument("--output")
parser.add_argument("--changed-file", action="append", default=[])
parser.add_argument("--removed-file", action="append", default=[])
args = parser.parse_args()
node_id = "sha256:" + hashlib.sha256(b"future-fixture-node").hexdigest()
records = [
    {"record": "lexicon", "schema_version": 1, "adapter_version": "future-fixture-v1", "language": "python", "repository": args.repo},
    {"record": "node", "id": node_id, "kind": "function", "path": "future.py", "name": "FutureThing", "qualified_name": "FutureThing", "owner": "future.py"},
    {"record": "unresolved", "source": node_id, "relation": "calls", "expression": "future_call()", "reason": "future-unresolved-reason", "owner": "future.py"},
]
destination = Path(args.output)
destination.parent.mkdir(parents=True, exist_ok=True)
destination.write_text("\\n".join(json.dumps(record, sort_keys=True, separators=(",", ":")) for record in records) + "\\n", encoding="utf-8")
""",
        encoding="utf-8",
    )

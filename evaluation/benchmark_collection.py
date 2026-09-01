from __future__ import annotations

from datetime import datetime, timezone
import json
from pathlib import Path
import subprocess
import time
from typing import Any

from benchmark_grounding import summarize_audit, validate_answer


def collect_runs(
    active: dict[str, tuple],
    *,
    checkout_by_condition: dict[str, Path],
    output: Path,
    expected_sections: list[str],
    required_path_prefixes: list[str],
    timeout_seconds: int = 2400,
) -> dict[str, dict[str, Any]]:
    results: dict[str, dict[str, Any]] = {}
    pending = set(active)
    while pending:
        for condition in list(pending):
            process, started, stdout_file, stderr_file, usage = active[condition]
            exit_code = process.poll()
            if exit_code is None and time.perf_counter() - started <= timeout_seconds:
                continue
            if exit_code is None:
                process.kill()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    pass
                exit_code = -9
            finished = time.perf_counter()
            stdout_file.close()
            stderr_file.close()
            usage_data = json.loads(usage.read_text(encoding="utf-8")) if usage.is_file() else None
            answer_path = output / f"{condition}.stdout.txt"
            audit_path = output / "grimoire.mcp-audit.jsonl" if condition == "grimoire" else None
            grounding = validate_answer(
                checkout_by_condition[condition],
                answer_path.read_text(encoding="utf-8") if answer_path.is_file() else "",
                exit_code=exit_code,
                expected_sections=expected_sections,
                audit_log=audit_path,
                require_grimoire_handles=False,
                required_path_prefixes=required_path_prefixes,
            )
            (output / f"{condition}.grounding.json").write_text(
                json.dumps(grounding.to_dict(), indent=2) + "\n", encoding="utf-8"
            )
            results[condition] = {
                "exit_code": exit_code,
                "elapsed_seconds": round(finished - started, 3),
                "usage": usage_data,
                "answer_bytes": answer_path.stat().st_size if answer_path.is_file() else 0,
                "discovery_output": summarize_audit(audit_path),
                "grounding": grounding.to_dict(),
                "execution_valid": exit_code == 0,
                "grounding_valid": grounding.valid,
                "eligible_for_scoring": exit_code == 0 and grounding.valid,
                "quality_assessed": False,
                "completed_at": datetime.now(timezone.utc).isoformat(),
            }
            pending.remove(condition)
        if pending:
            time.sleep(1)
    return results

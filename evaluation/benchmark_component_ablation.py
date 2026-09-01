from __future__ import annotations

import os
from pathlib import Path
import shutil
import time
from typing import Any

from benchmark_process import isolated_path, run_checked


def prewarm_lexicon_arcana(
    *,
    checkout: Path,
    output: Path,
    lexicon_binary: Path,
    arcana_binary: Path,
    adapters: Path,
    blocked_paths: list[Path],
) -> tuple[dict[str, Any], dict[str, str]]:
    export_dir = output / "lexicon-arcana-export"
    bin_dir = output / "lexicon-arcana-bin"
    shutil.rmtree(export_dir, ignore_errors=True)
    shutil.rmtree(bin_dir, ignore_errors=True)
    bin_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(lexicon_binary, bin_dir / lexicon_binary.name)
    shutil.copy2(arcana_binary, bin_dir / arcana_binary.name)

    environment = os.environ.copy()
    environment["PATH"] = isolated_path(lexicon_binary.parent, blocked_paths)
    lexicon_seconds, lexicon = _timed([
        str(lexicon_binary), "init", "--repo", str(checkout),
        "--adapters", str(adapters), "--languages", "all",
    ], checkout, environment, 3600)
    _record(output, "lexicon-arcana-lexicon-init", lexicon)

    arcana_seconds, arcana = _timed([
        str(arcana_binary), "sync",
        "--lexicon", str(checkout / ".lexicon"),
        "--state", str(checkout / ".arcana"),
    ], checkout, environment, 1800)
    _record(output, "lexicon-arcana-arcana-sync", arcana)

    export_seconds, exported = _timed([
        str(lexicon_binary), "export", "--repo", str(checkout),
        "--output", str(export_dir),
    ], checkout, environment, 1800)
    _record(output, "lexicon-arcana-export", exported)

    lexicon_snapshot = _read_current(checkout / ".lexicon")
    arcana_snapshot_id = _read_current(checkout / ".arcana")
    arcana_snapshot = checkout / ".arcana" / "snapshots" / arcana_snapshot_id.removeprefix("sha256:")
    if not (arcana_snapshot / "repository.manifest").is_file():
        raise RuntimeError(f"prepared Arcana snapshot is incomplete: {arcana_snapshot}")
    if lexicon_snapshot != arcana_snapshot_id:
        raise RuntimeError(
            "Lexicon and Arcana snapshots are not aligned after direct preparation: "
            f"{lexicon_snapshot} != {arcana_snapshot_id}"
        )

    export_bytes = sum(path.stat().st_size for path in export_dir.rglob("*") if path.is_file())
    summary = {
        "elapsed_seconds": round(lexicon_seconds + arcana_seconds + export_seconds, 3),
        "lexicon_seconds": round(lexicon_seconds, 3),
        "arcana_seconds": round(arcana_seconds, 3),
        "export_seconds": round(export_seconds, 3),
        "lexicon_snapshot": lexicon_snapshot,
        "arcana_snapshot": arcana_snapshot_id,
        "export_bytes": export_bytes,
    }
    return summary, {
        "export_dir": str(export_dir),
        "arcana_snapshot": str(arcana_snapshot),
        "bin_dir": str(bin_dir),
    }


def _timed(command: list[str], cwd: Path, env: dict[str, str], timeout: int):
    started = time.perf_counter()
    result = run_checked(command, cwd=cwd, env=env, timeout=timeout)
    return time.perf_counter() - started, result


def _record(output: Path, name: str, result) -> None:
    (output / f"{name}.stdout.txt").write_text(result.stdout, encoding="utf-8")
    (output / f"{name}.stderr.txt").write_text(result.stderr, encoding="utf-8")


def _read_current(state: Path) -> str:
    return (state / "CURRENT").read_text(encoding="utf-8").strip()

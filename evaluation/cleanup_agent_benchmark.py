from __future__ import annotations

import argparse
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = ROOT / "evaluation" / "results" / "agent-benchmark-v2"


def main() -> int:
    parser = argparse.ArgumentParser(description="Remove transient benchmark caches and partial summaries.")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--task", action="append", default=[])
    args = parser.parse_args()

    output = args.output.resolve()
    task_dirs = [output / task for task in args.task] if args.task else [path for path in output.iterdir() if path.is_dir()]
    removed: list[str] = []
    for task_dir in task_dirs:
        for name in ("cbm-cache", "lexicon-arcana-export", "lexicon-arcana-bin"):
            transient = task_dir / name
            if transient.exists():
                shutil.rmtree(transient)
                removed.append(str(transient))
    partial = output / "summary.partial.json"
    if partial.exists():
        partial.unlink()
        removed.append(str(partial))
    for path in removed:
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

"""Profile production Lexicon scans on a disposable Python or pinned Git tree."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import tempfile

from incremental_phase0_support import measure, seed_source


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--adapters", type=Path, required=True)
    parser.add_argument("--source", type=Path, help="Git repository; archives HEAD only")
    parser.add_argument("--synthetic-count", type=int, default=60)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=150)
    parser.add_argument("--cold-timeout", type=int)
    parser.add_argument("--max-steps", type=int, default=8)
    parser.add_argument("--interrupt", action="store_true")
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    result = {
        "lexicon_revision": subprocess.check_output(
            ["git", "-C", str(Path(__file__).resolve().parents[2]), "rev-parse", "HEAD"],
            text=True,
        ).strip(),
        "binary_version": subprocess.check_output(
            [str(args.binary), "version"], text=True
        ).strip(),
        "steps": {},
    }

    with tempfile.TemporaryDirectory(prefix="lexicon-phase0-") as disposable:
        repo = Path(disposable) / "source"
        result["source_revision"] = seed_source(
            repo, args.source, args.synthetic_count
        )
        python_files = sorted(repo.rglob("*.py"))
        if not python_files:
            raise RuntimeError("benchmark needs a Python file")
        original = python_files[0]
        mirror = repo / ".lexicon" / "repo" / "source" / original.relative_to(repo)
        steps = [
            ("init_full", ["init", "--repo", str(repo),
                           "--adapters", str(args.adapters), "--languages", "python"]),
            ("unchanged", ["scan", "--repo", str(repo)]),
            ("edit_once", ["scan", "--repo", str(repo)]),
            ("edit_twice", ["scan", "--repo", str(repo)]),
            ("dirty_mirror", ["scan", "--repo", str(repo)]),
            ("add_file", ["scan", "--repo", str(repo)]),
            ("remove_file", ["scan", "--repo", str(repo)]),
            ("rename_file", ["scan", "--repo", str(repo)]),
        ]
        added = original.parent / "phase0_added.py"
        renamed = original.with_name("phase0_renamed.py")
        for index, (label, command) in enumerate(steps):
            if index >= args.max_steps:
                break
            if label in ("edit_once", "edit_twice"):
                with original.open("a", encoding="utf-8") as output:
                    output.write(f"\n# phase0 {label}\n")
            elif label == "dirty_mirror":
                mirror.parent.mkdir(parents=True, exist_ok=True)
                with mirror.open("a", encoding="utf-8") as output:
                    output.write("\n# intentionally dirty private mirror\n")
            elif label == "add_file":
                added.write_text("added = True\n", encoding="utf-8")
            elif label == "remove_file":
                added.unlink()
            elif label == "rename_file":
                original.rename(renamed)
            timeout = args.cold_timeout or args.timeout if label == "init_full" else args.timeout
            record = measure(
                args.binary, command, timeout,
                args.output.with_suffix(f".{label}.log"),
            )
            current = repo / ".lexicon" / "CURRENT"
            record["snapshot_id"] = (
                current.read_text(encoding="utf-8").strip()
                if current.exists() else None
            )
            result["steps"][label] = record
            args.output.write_text(json.dumps(result, indent=2), encoding="utf-8")
            if record["exit_code"] != 0 or record["timed_out"]:
                break

        if args.interrupt and all(
            step["exit_code"] == 0 and not step["timed_out"]
            for step in result["steps"].values()
        ):
            with renamed.open("a", encoding="utf-8") as output:
                output.write("\n# interruption probe\n")
            result["steps"]["interrupt"] = measure(
                args.binary, ["scan", "--repo", str(repo)], 1,
                args.output.with_suffix(".interrupt.log"),
            )
            result["steps"]["recover"] = measure(
                args.binary, ["scan", "--repo", str(repo)], args.timeout,
                args.output.with_suffix(".recover.log"),
            )
        args.output.write_text(json.dumps(result, indent=2), encoding="utf-8")
    return int(any(
        step["timed_out"] or step["exit_code"] != 0
        for label, step in result["steps"].items()
        if label != "interrupt"
    ))


if __name__ == "__main__":
    raise SystemExit(main())

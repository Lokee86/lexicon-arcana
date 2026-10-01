"""Bounded production CLI acceptance on an exact pinned Hermes Git revision."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import time

from incremental_phase0_support import measure
from incremental_phase5_fixture import PINNED_HERMES, facts_digest, prepare, provenance

ALL = (
    "init_full", "unchanged", "edit_once", "edit_twice",
    "dirty_mirror", "edit_ten", "add_file", "remove_file", "rename_file",
)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--binary", required=True, type=Path)
    ap.add_argument("--source", required=True, type=Path)
    ap.add_argument("--adapters", required=True, type=Path)
    ap.add_argument("--output", required=True, type=Path)
    ap.add_argument("--role", required=True, choices=("before", "after"))
    ap.add_argument("--subset-python", type=int, default=0)
    ap.add_argument("--source-path", default=None, help="pinned package path")
    ap.add_argument("--steps", default=",".join(ALL))
    ap.add_argument("--cold-timeout", type=int, default=240)
    ap.add_argument("--step-timeout", type=int, default=90)
    ap.add_argument("--rss-cap-mib", type=int, default=2250)
    ap.add_argument("--interrupt", action="store_true")
    ap.add_argument("--export", action="store_true")
    ap.add_argument("--total-limit", type=int, default=1200)
    args = ap.parse_args()
    labels = [label.strip() for label in args.steps.split(",") if label.strip()]
    if not labels or labels[0] != "init_full" or any(x not in ALL for x in labels):
        ap.error("--steps must begin with init_full and contain only known steps")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    output = args.output.resolve()
    report = {
        "role": args.role,
        "source_revision": PINNED_HERMES,
        "binary": str(args.binary),
        "provenance": provenance(args.binary),
        "run_status": "incomplete",
        "steps": {},
    }

    def save() -> None:
        output.write_text(json.dumps(report, indent=2), encoding="utf-8")

    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix=f"lexicon-phase5-{args.role}-") as tmp:
        repo = Path(tmp) / "source"
        report["fixture"] = prepare(repo, args.source, args.subset_python, args.source_path)
        original = repo / report["fixture"]["original"]
        ten = [repo / path for path in report["fixture"]["ten_files"]]
        mirror = repo / ".lexicon" / "repo" / "source" / original.relative_to(repo)
        added = original.parent / "phase5_added.py"
        renamed = original.with_name("phase5_renamed_" + original.name)
        export_dir = Path(tmp) / "export"
        save()

        for label in labels:
            if time.monotonic() - started > args.total_limit:
                report["stop_reason"] = "total_wall_limit"
                break
            if label in ("edit_once", "edit_twice"):
                with original.open("a", encoding="utf-8") as out:
                    out.write(f"\n# phase5 {label}\n")
            elif label == "dirty_mirror":
                with mirror.open("a", encoding="utf-8") as out:
                    out.write("\n# phase5 private mirror repair\n")
            elif label == "edit_ten":
                for n, path in enumerate(ten):
                    with path.open("a", encoding="utf-8") as out:
                        out.write(f"\n# phase5 batch {n}\n")
            elif label == "add_file":
                added.write_text("phase5_added = True\n", encoding="utf-8")
            elif label == "remove_file":
                added.unlink()
            elif label == "rename_file":
                original.rename(renamed)

            command = (["init", "--repo", str(repo), "--adapters",
                        str(args.adapters), "--languages", "python"]
                       if label == "init_full" else ["scan", "--repo", str(repo)])
            record = measure(
                args.binary, command,
                args.cold_timeout if label == "init_full" else args.step_timeout,
                output.with_suffix(f".{label}.log"),
                args.rss_cap_mib * 1024 * 1024,
            )
            current = repo / ".lexicon" / "CURRENT"
            record["snapshot_id"] = current.read_text(encoding="utf-8").strip() \
                if current.exists() else None
            report["steps"][label] = record
            if args.export and record["exit_code"] == 0 and not record["timed_out"]:
                export_dir.mkdir(exist_ok=True)
                exported = subprocess.run(
                    [str(args.binary), "export", "--repo", str(repo),
                     "--output", str(export_dir), "--languages", "python"],
                    capture_output=True, text=True, timeout=args.step_timeout,
                )
                record["export_exit"] = exported.returncode
                record["export_error"] = exported.stderr[-600:]
                if exported.returncode == 0:
                    record["facts"] = facts_digest(export_dir)
            save()
            if (record["exit_code"] != 0 or record["timed_out"]
                    or record["rss_limit_exceeded"]
                    or args.export and record.get("export_exit") != 0):
                report["stop_reason"] = f"{label}: failed, capped or timed out"
                break
        else:
            report["run_status"] = "completed"

        if args.interrupt and report["run_status"] == "completed":
            target = renamed if renamed.exists() else original
            with target.open("a", encoding="utf-8") as out:
                out.write("\n# phase5 cancellation probe\n")
            report["steps"]["interrupt"] = measure(
                args.binary, ["scan", "--repo", str(repo)], 1,
                output.with_suffix(".interrupt.log"),
                args.rss_cap_mib * 1024 * 1024,
            )
            # Real lock-release and recovery checks, not just a process kill.
            for label in ("recover", "post_recovery_unchanged"):
                record = measure(
                    args.binary, ["scan", "--repo", str(repo)], args.step_timeout,
                    output.with_suffix(f".{label}.log"),
                    args.rss_cap_mib * 1024 * 1024,
                )
                current = repo / ".lexicon" / "CURRENT"
                record["snapshot_id"] = current.read_text(encoding="utf-8").strip() \
                    if current.exists() else None
                report["steps"][label] = record
                save()
                if record["exit_code"] or record["timed_out"]:
                    report["run_status"] = "recovery_failed"
                    break
            if report["run_status"] == "completed":
                a = report["steps"]["recover"]["snapshot_id"]
                b = report["steps"]["post_recovery_unchanged"]["snapshot_id"]
                if a != b:
                    report["run_status"] = "recovery_not_stable"
            save()
    report["elapsed_seconds"] = round(time.monotonic() - started, 2)
    save()
    print("PHASE5", args.role, report["run_status"],
          "completed_steps", list(report["steps"]), "seconds", report["elapsed_seconds"],
          flush=True)
    return 0 if report["run_status"] == "completed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
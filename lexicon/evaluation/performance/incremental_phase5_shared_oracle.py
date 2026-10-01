"""Independent full-fact oracle for the pinned acp_adapter one-comment edit."""
from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path
import subprocess
import tempfile

from incremental_phase0_support import measure
from incremental_phase5_fixture import prepare

def export(binary: Path, repo: Path, folder: Path) -> Counter[str]:
    folder.mkdir()
    result = subprocess.run(
        [str(binary), "export", "--repo", str(repo), "--output", str(folder),
         "--languages", "python"],
        capture_output=True, text=True, timeout=30,
    )
    if result.returncode:
        raise RuntimeError(f"export failed: {result.stderr[-350:]}")
    with (folder / "python.jsonl").open("r", encoding="utf-8") as input_:
        next(input_)
        return Counter(line.rstrip("\n") for line in input_)


def case(name: str, binary: Path, direct_full: bool, report: dict,
         source: Path, adapters: Path, output: Path) -> Counter[str]:
    with tempfile.TemporaryDirectory(prefix=f"lexicon-phase5-oracle-{name}-") as tmp:
        directory = Path(tmp)
        repo = directory / "source"
        fixture = prepare(repo, source, 0, "acp_adapter")
        edited = repo / fixture["original"]
        if direct_full:
            with edited.open("a", encoding="utf-8") as file:
                file.write("\n# phase5 edit_once\n")
        init = measure(binary, ["init", "--repo", str(repo), "--adapters",
                                str(adapters), "--languages", "python"],
                       75, directory / "init.log", 1500 * 1024 * 1024)
        entry = {"init_seconds": init["wall_seconds"],
                 "init_exit": init["exit_code"],
                 "init_timed_out": init["timed_out"]}
        report[name] = entry
        output.write_text(json.dumps(report, indent=2), encoding="utf-8")
        if init["exit_code"] or init["timed_out"]:
            raise RuntimeError(f"{name} init failed: {init['stderr_tail'][-350:]}")
        if not direct_full:
            with edited.open("a", encoding="utf-8") as file:
                file.write("\n# phase5 edit_once\n")
            edit = measure(binary, ["scan", "--repo", str(repo)], 60,
                           directory / "edit.log", 1500 * 1024 * 1024)
            entry.update({"edit_seconds": edit["wall_seconds"],
                          "edit_exit": edit["exit_code"],
                          "edit_timed_out": edit["timed_out"],
                          "shared_merge": [x for x in edit["metrics"]
                                           if x.get("stage") in (
                                               "scan.shared_merge",
                                               "scan.adapter.python.full",
                                               "scan.adapter.python.incremental")]})
            output.write_text(json.dumps(report, indent=2), encoding="utf-8")
            if edit["exit_code"] or edit["timed_out"]:
                raise RuntimeError(f"{name} scan failed: {edit['stderr_tail'][-350:]}")
        return export(binary, repo, directory / "export")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--adapters", type=Path, required=True)
    parser.add_argument("--before", type=Path, required=True)
    parser.add_argument("--after", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report = {"source_revision": "7798df0a83721df7ff44b9ba023d56b85b351d1e",
              "change": "append # phase5 edit_once to acp_adapter/__init__.py",
              "cases": {}}
    counts = {}
    # Full edited analysis is the independent semantic oracle, not either
    # incremental implementation's result.
    for name, binary, direct_full in [
        ("edited_full_oracle", args.after, True),
        ("before_incremental", args.before, False),
        ("after_incremental", args.after, False),
    ]:
        print("CASE", name, flush=True)
        counts[name] = case(name, binary, direct_full, report["cases"],
                            args.source, args.adapters, args.output)
        report["cases"][name]["fact_count"] = sum(counts[name].values())
        args.output.write_text(json.dumps(report, indent=2), encoding="utf-8")
    oracle = counts["edited_full_oracle"]
    for name in ("before_incremental", "after_incremental"):
        missing = oracle - counts[name]
        extra = counts[name] - oracle
        report["cases"][name]["missing_vs_full"] = [
            {"count": count, "fact": json.loads(line)}
            for line, count in missing.items()
        ][:4]
        report["cases"][name]["extra_vs_full"] = [
            {"count": count, "fact": json.loads(line)}
            for line, count in extra.items()
        ][:4]
        report["cases"][name]["matches_full_oracle"] = not missing and not extra
    args.output.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print("ORACLE",
          {name: {"count": item["fact_count"],
                  "matches": item.get("matches_full_oracle"),
                  "missing": len(item.get("missing_vs_full", [])),
                  "extra": len(item.get("extra_vs_full", []))}
           for name, item in report["cases"].items()}, flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
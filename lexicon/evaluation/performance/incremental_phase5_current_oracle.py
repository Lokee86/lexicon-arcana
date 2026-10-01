"""Verify repaired incremental exports against independent edited full scans."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import tempfile

from incremental_phase5_fixture import facts_digest, prepare


def independent_full(binary: Path, source: Path, adapters: Path, edits: list[str]) -> dict:
    with tempfile.TemporaryDirectory(prefix="lexicon-phase5-oracle-") as directory:
        parent = Path(directory)
        repo = parent / "source"
        fixture = prepare(repo, source, 0, "acp_adapter")
        file = repo / fixture["original"]
        with file.open("a", encoding="utf-8") as handle:
            for label in edits:
                handle.write(f"\n# phase5 {label}\n")
        result = subprocess.run(
            [str(binary), "init", "--repo", str(repo), "--adapters",
             str(adapters), "--languages", "python"],
            capture_output=True, text=True, timeout=65,
        )
        if result.returncode:
            raise RuntimeError(f"independent full init failed: {result.stderr[-600:]}")
        output = parent / "export"
        output.mkdir()
        result = subprocess.run(
            [str(binary), "export", "--repo", str(repo), "--output",
             str(output), "--languages", "python"],
            capture_output=True, text=True, timeout=35,
        )
        if result.returncode:
            raise RuntimeError(f"independent export failed: {result.stderr[-600:]}")
        return facts_digest(output)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--adapters", type=Path, required=True)
    parser.add_argument("--benchmark", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    measured = json.loads(args.benchmark.read_text())
    evidence = {}
    for name, edits in [
        ("edit_once", ["edit_once"]),
        ("edit_twice", ["edit_once", "edit_twice"]),
    ]:
        expected = independent_full(args.binary, args.source, args.adapters, edits)
        actual = measured["steps"][name]["facts"]
        exact = (actual["records_sha256"] == expected["records_sha256"]
                 and actual["fact_record_count"] == expected["fact_record_count"])
        evidence[name] = {
            "matches_independent_full": exact,
            "independent_full": expected,
            "incremental": actual,
        }
        print(name, "PASS" if exact else "FAIL", flush=True)
        args.output.write_text(json.dumps(evidence, indent=2))
    return int(not all(v["matches_independent_full"] for v in evidence.values()))


if __name__ == "__main__":
    raise SystemExit(main())

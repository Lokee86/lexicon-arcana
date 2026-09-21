"""Command-line entry point for the Lexicon Python adapter."""

from __future__ import annotations

import argparse
from pathlib import Path

from .adapter import write_facts


def main() -> int:
    parser = argparse.ArgumentParser(description="Emit Lexicon facts v1 for a Python repository")
    parser.add_argument("--repo", required=True, type=Path, help="repository root to scan")
    parser.add_argument("--output", required=True, type=Path, help="JSONL output path (use - for stdout)")
    parser.add_argument("--changed-file", action="append", dest="changed_files")
    parser.add_argument("--removed-file", action="append", dest="removed_files")
    parser.add_argument("--workers", type=int, default=1, help="generic execution-plan worker budget")
    parser.add_argument("--shards", type=int, default=1, help="generic execution-plan shard count")
    parser.add_argument("--merge-fan-in", type=int, default=2, dest="merge_fan_in", help="generic execution-plan merge fan-in")
    args = parser.parse_args()
    write_facts(args.repo, args.output, args.changed_files, args.removed_files, args.workers, args.shards, args.merge_fan_in)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

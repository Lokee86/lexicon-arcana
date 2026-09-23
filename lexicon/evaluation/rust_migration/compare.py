from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def compare(kind: str, left: Path, right: Path) -> tuple[bool, dict[str, Any]]:
    left_bytes = left.read_bytes()
    right_bytes = right.read_bytes()
    detail = {
        "left_sha256": digest(left_bytes),
        "right_sha256": digest(right_bytes),
        "left_bytes": len(left_bytes),
        "right_bytes": len(right_bytes),
    }
    if kind in {"bytes", "jsonl"}:
        return left_bytes == right_bytes, detail
    if kind == "json":
        equal = json.loads(left_bytes) == json.loads(right_bytes)
        return equal, detail
    raise ValueError(f"unsupported artifact kind: {kind}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("candidate", type=Path)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path(__file__).with_name("reference.json"),
    )
    args = parser.parse_args()

    manifest = load_json(args.manifest)
    fixture_root = args.manifest.parent / "fixtures"
    results = []
    failed = False
    for artifact in manifest["captured_fixtures"]:
        reference = fixture_root / artifact["path"]
        candidate = args.candidate / artifact["path"]
        if not candidate.is_file():
            results.append({"name": artifact["name"], "status": "missing"})
            failed = True
            continue
        equal, detail = compare(artifact["kind"], reference, candidate)
        results.append(
            {
                "name": artifact["name"],
                "kind": artifact["kind"],
                "status": "match" if equal else "different",
                **detail,
            }
        )
        failed |= not equal

    print(
        json.dumps(
            {
                "reference_commit": manifest["reference_commit"],
                "results": results,
            },
            indent=2,
            sort_keys=True,
        )
    )
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())

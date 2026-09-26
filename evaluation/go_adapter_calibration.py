#!/usr/bin/env python3
import argparse
import collections
import hashlib
import json
from pathlib import Path


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def read_stream(path):
    raw_hash = hashlib.sha256()
    semantic_hash = hashlib.sha256()
    records = collections.Counter()
    stats = {
        "records": 0,
        "record_types": collections.Counter(),
        "node_kinds": collections.Counter(),
        "edge_relations": collections.Counter(),
        "unresolved_reasons": collections.Counter(),
    }
    with Path(path).open("rb") as handle:
        first = handle.readline()
        raw_hash.update(first)
        header = json.loads(first)
        semantic_header = dict(header)
        semantic_header.pop("adapter_version", None)
        semantic_hash.update(canonical(semantic_header).encode())
        semantic_hash.update(b"\n")
        for line in handle:
            raw_hash.update(line)
            if not line.strip():
                continue
            value = json.loads(line)
            encoded = canonical(value)
            records[encoded] += 1
            stats["records"] += 1
            record_type = value["record"]
            stats["record_types"][record_type] += 1
            if record_type == "node":
                stats["node_kinds"][value["kind"]] += 1
            elif record_type == "edge":
                stats["edge_relations"][value["relation"]] += 1
            elif record_type == "unresolved":
                stats["unresolved_reasons"][value["reason"]] += 1

    for encoded, count in sorted(records.items()):
        semantic_hash.update(encoded.encode())
        semantic_hash.update(b"\x00")
        semantic_hash.update(str(count).encode())
        semantic_hash.update(b"\n")
    return {
        "path": str(Path(path)),
        "header": header,
        "semantic_header": semantic_header,
        "raw_sha256": raw_hash.hexdigest(),
        "semantic_sha256": semantic_hash.hexdigest(),
        "records": records,
        "stats": stats,
    }


def plain_stats(stats):
    return {
        key: dict(sorted(value.items())) if isinstance(value, collections.Counter) else value
        for key, value in stats.items()
    }


def sample(counter, limit):
    result = []
    for encoded, count in counter.most_common():
        result.append({"count": count, "record": json.loads(encoded)})
        if len(result) >= limit:
            break
    return result


def compare(left_path, right_path, sample_limit):
    left = read_stream(left_path)
    right = read_stream(right_path)
    left_only = left["records"] - right["records"]
    right_only = right["records"] - left["records"]
    semantic_header_equal = left["semantic_header"] == right["semantic_header"]
    report = {
        "equal": semantic_header_equal and not left_only and not right_only,
        "semantic_header_equal": semantic_header_equal,
        "left": {
            "path": left["path"],
            "header": left["header"],
            "raw_sha256": left["raw_sha256"],
            "semantic_sha256": left["semantic_sha256"],
            "stats": plain_stats(left["stats"]),
        },
        "right": {
            "path": right["path"],
            "header": right["header"],
            "raw_sha256": right["raw_sha256"],
            "semantic_sha256": right["semantic_sha256"],
            "stats": plain_stats(right["stats"]),
        },
        "left_only_records": sum(left_only.values()),
        "right_only_records": sum(right_only.values()),
        "left_only_sample": sample(left_only, sample_limit),
        "right_only_sample": sample(right_only, sample_limit),
    }
    return report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("left")
    parser.add_argument("right")
    parser.add_argument("--samples", type=int, default=12)
    parser.add_argument("--output")
    args = parser.parse_args()
    report = compare(args.left, args.right, args.samples)
    output = json.dumps(report, indent=2, sort_keys=True)
    if args.output:
        Path(args.output).write_text(output + "\n", encoding="utf-8")
    print(output)
    raise SystemExit(0 if report["equal"] else 2)


if __name__ == "__main__":
    main()

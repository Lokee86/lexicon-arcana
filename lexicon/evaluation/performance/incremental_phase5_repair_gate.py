"""Fail-closed gates for pinned Phase 5 repair fixtures (NOT full acceptance)."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

PACKAGE = ("init_full", "unchanged", "edit_once", "edit_twice")
PINNED = "7798df0a83721df7ff44b9ba023d56b85b351d1e"


def inspect(prior: dict, repaired: dict, oracle: dict, agent: dict) -> list[str]:
    errors: list[str] = []
    def require(ok: bool, message: str) -> None:
        if not ok:
            errors.append(message)

    require(prior.get("source_revision") == PINNED, "prior revision mismatch")
    require(repaired.get("source_revision") == PINNED, "repaired revision mismatch")
    require(agent.get("source_revision") == PINNED, "agent revision mismatch")
    require(prior.get("fixture") == repaired.get("fixture"), "package fixture differs")
    require(repaired.get("run_status") == "completed", "package incomplete")
    for label in PACKAGE:
        old = prior.get("steps", {}).get(label, {})
        new = repaired.get("steps", {}).get(label, {})
        require(old.get("exit_code") == 0 and new.get("exit_code") == 0,
                f"{label}: censored or missing paired result")
        prior_facts, new_facts = old.get("facts", {}), new.get("facts", {})
        require(bool(prior_facts.get("records_sha256"))
                and prior_facts.get("records_sha256") == new_facts.get("records_sha256"),
                f"{label}: corrected baseline fact parity")
        budget = 5 if label == "unchanged" else 10 if "edit" in label else None
        if budget:
            require(new.get("wall_seconds", float("inf")) <= budget,
                    f"{label}: package latency exceeds {budget}s")

    for label in ("edit_once", "edit_twice"):
        metrics = repaired.get("steps", {}).get(label, {}).get("metrics", [])
        stage = lambda name: [m for m in metrics if m.get("stage") == name]
        require(not stage("scan.adapter.python.full"), f"{label}: unnecessary full retry")
        require(not stage("scan.dependency_bootstrap"), f"{label}: index bootstrap")
        require(len(stage("scan.dependency_index_query")) == 1
                and stage("scan.dependency_index_query")[0].get("fact_object_reads") == 0,
                f"{label}: dependency query read prior facts")
        gold = oracle.get(label, {})
        require(gold.get("matches_independent_full") is True
                and gold.get("independent_full", {}).get("records_sha256")
                == repaired.get("steps", {}).get(label, {}).get("facts", {}).get("records_sha256"),
                f"{label}: no independent full-source parity")

    require(agent.get("fixture", {}).get("fixture") == "pinned-hermes-path:agent",
            "wrong agent source subset")
    require(agent.get("fixture", {}).get("python_count") == 302,
            "agent file-count mismatch")
    full = agent.get("steps", {}).get("init_full", {})
    require(full.get("exit_code") == 0 and not full.get("timed_out")
            and bool(full.get("snapshot_id")), "agent cold scan incomplete")
    require(full.get("wall_seconds", float("inf")) <= 85, "agent bounded cold timeout")
    stages = full.get("metrics", [])
    require(sum(m.get("stage") == "python.extract_shard" for m in stages) == 8
            and sum(m.get("stage") == "python.extract_shard_start" for m in stages) == 8,
            "agent extraction shard coverage incomplete")
    require(any(m.get("stage") == "python.final_fact_emission"
                and m.get("final_fact_count") == 249297 for m in stages),
            "agent Python fact emission incomplete")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("prior", "repaired", "oracle", "agent"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    reports = [json.loads(getattr(args, name).read_text(encoding="utf-8"))
               for name in ("prior", "repaired", "oracle", "agent")]
    errors = inspect(*reports)
    print("PHASE 5 REPAIR FIXTURES:", "PASS" if not errors else "FAIL")
    for error in errors:
        print("-", error)
    print("Full 7,114-file Hermes acceptance is NOT covered.")
    return bool(errors)


if __name__ == "__main__":
    raise SystemExit(main())

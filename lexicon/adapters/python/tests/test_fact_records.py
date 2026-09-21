from __future__ import annotations

from lexicon_python.emission import emit_records
from lexicon_python.model import Facts


def test_compact_fact_records_round_trip() -> None:
    facts = Facts(repository="demo")
    node_id = facts.add_node(
        "function",
        "run",
        "src/demo.py",
        "demo.run",
        record_span={
            "path": "src/demo.py",
            "start_line": 3,
            "start_column": 1,
            "end_line": 4,
            "end_column": 5,
        },
        attributes={"decorators": ["cache"], "async": True},
    )
    facts.add_edge(
        node_id,
        "sha256:target",
        "calls",
        record_span={
            "path": "src/demo.py",
            "start_line": 4,
            "start_column": 2,
            "end_line": 4,
            "end_column": 9,
        },
        attributes={"candidate_count": 1},
    )
    facts.add_unresolved(
        node_id,
        "calls",
        "dynamic()",
        "dynamic-target",
        candidate_name="dynamic",
    )

    records = emit_records(facts, "test")
    assert records[1] == {
        "record": "node",
        "id": node_id,
        "kind": "function",
        "name": "run",
        "path": "src/demo.py",
        "qualified_name": "demo.run",
        "attributes": {"decorators": ["cache"], "async": True},
        "span": {
            "path": "src/demo.py",
            "start_line": 3,
            "start_column": 1,
            "end_line": 4,
            "end_column": 5,
        },
    }
    assert records[2]["attributes"] == {"candidate_count": 1}
    assert records[3]["candidate_name"] == "dynamic"


def test_compact_fact_records_deduplicate_identical_records() -> None:
    facts = Facts(repository="demo")
    for _ in range(2):
        facts.add_edge("source", "target", "calls")
        facts.add_unresolved(
            "source",
            "calls",
            "dynamic()",
            "dynamic-target",
            candidate_name="dynamic",
        )

    assert len(facts.edges) == 1
    assert len(facts.unresolved) == 1


def test_compact_fact_record_sort_ties_are_deterministic() -> None:
    facts = Facts(repository="demo")
    facts.add_edge(
        "source",
        "target",
        "possible-calls",
        attributes={"candidate_count": 2},
    )
    facts.add_edge(
        "source",
        "target",
        "possible-calls",
        attributes={"candidate_count": 1},
    )
    facts.add_unresolved(
        "source",
        "calls",
        "dynamic()",
        "dynamic-target",
        candidate_name="zeta",
    )
    facts.add_unresolved(
        "source",
        "calls",
        "dynamic()",
        "dynamic-target",
        candidate_name="alpha",
    )

    records = emit_records(facts, "test")
    edges = [record for record in records if record.get("record") == "edge"]
    unresolved = [
        record for record in records if record.get("record") == "unresolved"
    ]

    assert [record["attributes"]["candidate_count"] for record in edges] == [2, 1]
    assert [record["candidate_name"] for record in unresolved] == ["zeta", "alpha"]

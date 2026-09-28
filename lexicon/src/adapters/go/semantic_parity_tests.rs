use std::{fs, path::PathBuf};

use crate::{AdapterRequest, Analysis, FactRecord, LanguageAdapter};

use super::{GoAdapter, tests::real_helper};

#[test]
fn all_oracle_through_phase_fourteen_matches_legacy() {
    for name in [
        "basic_calls",
        "relationships",
        "higher_order",
        "dataflow",
        "build_tags",
        "multi_module",
        "parallel",
    ] {
        let repository = fixture(name);
        let adapter = GoAdapter::with_frontend(real_helper());
        let analysis = adapter
            .analyze(&AdapterRequest {
                language: "go".into(),
                repository,
                ..AdapterRequest::default()
            })
            .unwrap();
        analysis.validate().unwrap();

        let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("testdata/go_oracle/golden")
            .join(format!("{name}.jsonl"));
        let legacy = Analysis::parse(
            &fs::read_to_string(golden)
                .unwrap_or_else(|error| panic!("read {name} oracle: {error}")),
        )
        .unwrap();

        assert_eq!(
            phase_four_records(&analysis.records),
            phase_four_records(&legacy.records),
            "repository/declaration parity failed for {name}"
        );
        assert_eq!(
            phase_five_node_ids(&analysis.records),
            phase_five_node_ids(&legacy.records),
            "node identity parity failed for {name}"
        );
        assert_eq!(
            phase_seven_relationships(&analysis.records),
            phase_seven_relationships(&legacy.records),
            "typed relationship parity failed for {name}"
        );
        assert_no_implements_self_edges(&analysis.records, name);
        if name != "build_tags" {
            assert_eq!(
                phase_nine_calls(&analysis.records),
                phase_nine_calls(&legacy.records),
                "SSA/VTA call-target parity failed for {name}"
            );
        }
        assert_eq!(
            phase_ten_captures(&analysis.records),
            phase_ten_captures(&legacy.records),
            "closure capture parity failed for {name}"
        );
        assert_eq!(
            phase_eleven_dataflow(&analysis.records),
            phase_eleven_dataflow(&legacy.records),
            "typed dataflow parity failed for {name}"
        );
        assert_eq!(
            phase_twelve_dependencies(&analysis.records),
            phase_twelve_dependencies(&legacy.records),
            "dependency parity failed for {name}"
        );
        assert_eq!(
            analysis.header.repository, legacy.header.repository,
            "repository identity failed for {name}"
        );
    }
}

#[test]
fn native_parallel_execution_is_deterministic() {
    let repository = fixture("parallel");
    let adapter = GoAdapter::with_frontend(real_helper());
    let configurations = [(1, 1, 2), (4, 8, 4), (3, 6, 8)];
    let mut baseline = None;
    for (workers, shards, merge_fan_in) in configurations {
        let analysis = adapter
            .analyze(&AdapterRequest {
                language: "go".into(),
                repository: repository.clone(),
                workers,
                shards,
                merge_fan_in,
                ..AdapterRequest::default()
            })
            .unwrap();
        analysis.validate().unwrap();
        if let Some(expected) = baseline.as_ref() {
            assert_eq!(
                &analysis, expected,
                "native Go output changed for execution {workers}/{shards}/{merge_fan_in}"
            );
        } else {
            baseline = Some(analysis);
        }
    }
}

fn phase_four_records(records: &[FactRecord]) -> Vec<FactRecord> {
    let ids = records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node)
                if matches!(node.kind.as_str(), "repository" | "directory" | "file")
                    || (node.owner.is_some()
                        && matches!(
                            node.kind.as_str(),
                            "module" | "import" | "type" | "function" | "method" | "test"
                        )) =>
            {
                Some(node.id.clone())
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();

    records
        .iter()
        .filter(|record| match record {
            FactRecord::Node(node) => ids.contains(&node.id),
            FactRecord::Edge(edge) => {
                matches!(edge.relation.as_str(), "contains" | "defines" | "imports")
                    && ids.contains(&edge.source)
                    && ids.contains(&edge.target)
            }
            FactRecord::Unresolved(_) => false,
        })
        .cloned()
        .collect()
}

fn phase_five_node_ids(records: &[FactRecord]) -> Vec<(String, String)> {
    phase_four_records(records)
        .into_iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) => Some((node.kind, node.id)),
            _ => None,
        })
        .collect()
}

fn phase_seven_relationships(records: &[FactRecord]) -> Vec<FactRecord> {
    records
        .iter()
        .filter(|record| {
            matches!(
                record,
                FactRecord::Edge(edge)
                    if matches!(
                        edge.relation.as_str(),
                        "implements" | "extends" | "overrides"
                    )
            )
        })
        .cloned()
        .collect()
}

fn phase_nine_calls(records: &[FactRecord]) -> Vec<FactRecord> {
    records
        .iter()
        .filter(|record| match record {
            FactRecord::Edge(edge) => matches!(
                edge.relation.as_str(),
                "calls" | "possible-calls" | "converts-to"
            ),
            FactRecord::Unresolved(value) => value.relation == "calls",
            FactRecord::Node(_) => false,
        })
        .cloned()
        .collect()
}

fn phase_ten_captures(records: &[FactRecord]) -> Vec<FactRecord> {
    let targets = records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Edge(edge) if edge.relation == "references" => Some(edge.target.clone()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    records
        .iter()
        .filter(|record| match record {
            FactRecord::Node(node) => targets.contains(&node.id),
            FactRecord::Edge(edge) => edge.relation == "references",
            FactRecord::Unresolved(_) => false,
        })
        .cloned()
        .collect()
}

fn phase_eleven_dataflow(records: &[FactRecord]) -> Vec<FactRecord> {
    let targets = records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Edge(edge) if matches!(edge.relation.as_str(), "reads" | "writes") => {
                Some(edge.target.clone())
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    records
        .iter()
        .filter(|record| match record {
            FactRecord::Node(node) => targets.contains(&node.id),
            FactRecord::Edge(edge) => matches!(edge.relation.as_str(), "reads" | "writes"),
            FactRecord::Unresolved(_) => false,
        })
        .cloned()
        .collect()
}

fn phase_twelve_dependencies(records: &[FactRecord]) -> Vec<FactRecord> {
    let targets = records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Edge(edge) if edge.relation == "depends-on" => Some(edge.target.clone()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    records
        .iter()
        .filter(|record| match record {
            FactRecord::Node(node) => targets.contains(&node.id),
            FactRecord::Edge(edge) => edge.relation == "depends-on",
            FactRecord::Unresolved(_) => false,
        })
        .cloned()
        .collect()
}

fn assert_no_implements_self_edges(records: &[FactRecord], fixture: &str) {
    for record in records {
        if let FactRecord::Edge(edge) = record
            && edge.relation == "implements"
        {
            assert_ne!(
                edge.source, edge.target,
                "implements self-edge emitted for {fixture}: {}",
                edge.source
            );
        }
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/go_oracle/repositories")
        .join(name)
}

use std::{fs, path::PathBuf};

use crate::{AdapterRequest, Analysis, FactRecord, LanguageAdapter};

use super::{GoAdapter, tests::real_helper};

#[test]
fn all_oracle_structural_declarations_match_legacy() {
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
        let adapter = GoAdapter::with_helper(real_helper());
        let analysis = adapter
            .analyze(&AdapterRequest {
                language: "go".into(),
                repository,
                ..AdapterRequest::default()
            })
            .unwrap();
        analysis.validate().unwrap();

        let golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("adapters/go/testdata/oracle_golden")
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
            analysis.header.repository, legacy.header.repository,
            "repository identity failed for {name}"
        );
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

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("adapters/go/testdata/oracle")
        .join(name)
}

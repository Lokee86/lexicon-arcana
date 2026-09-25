use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{AdapterRequest, Analysis, FactRecord, LanguageAdapter};

use super::{GoAdapter, discovery, tests::synthetic_helper};

#[test]
fn basic_oracle_discovery_and_structural_facts_match_legacy() {
    let repository = fixture("basic_calls");
    let inventory = discovery::discover(&repository).unwrap();

    assert_eq!(inventory.repository, "example.com/oracle/basic");
    assert_eq!(
        inventory.directories,
        vec!["internal".to_string(), "internal/sub".to_string()]
    );
    assert_eq!(
        inventory.semantic_files(),
        vec![
            "go.mod".to_string(),
            "internal/sub/sub.go".to_string(),
            "main.go".to_string(),
        ]
    );
    assert_eq!(
        inventory.modules,
        vec![discovery::Module {
            root: ".".into(),
            path: "example.com/oracle/basic".into(),
        }]
    );
    assert_eq!(
        inventory
            .module_for_file("internal/sub/sub.go")
            .unwrap()
            .path,
        "example.com/oracle/basic"
    );

    let helper_root = TempDirectory::new("oracle-helper");
    let adapter = GoAdapter::with_helper(synthetic_helper(
        &helper_root.path,
        r#"{"protocol_version":1,"records":[]}"#,
    ));
    let analysis = adapter
        .analyze(&AdapterRequest {
            language: "go".into(),
            repository,
            ..AdapterRequest::default()
        })
        .unwrap();
    analysis.validate().unwrap();

    let legacy = Analysis::parse(include_str!(
        "../../../adapters/go/testdata/oracle_golden/basic_calls.jsonl"
    ))
    .unwrap();
    assert_eq!(analysis.records, structural_records(&legacy.records));
}

#[test]
fn all_oracle_structural_facts_match_legacy() {
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
        let inventory = discovery::discover(&repository).unwrap();
        let analysis = super::facts::structural_analysis(
            &AdapterRequest {
                language: "go".into(),
                repository,
                ..AdapterRequest::default()
            },
            &inventory,
        )
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
            analysis.records,
            structural_records(&legacy.records),
            "structural parity failed for {name}"
        );
        assert_eq!(
            analysis.header.repository, legacy.header.repository,
            "repository identity failed for {name}"
        );
    }
}

#[test]
fn multi_module_repository_uses_nearest_go_mod() {
    let inventory = discovery::discover(&fixture("multi_module")).unwrap();

    assert_eq!(inventory.repository, "multi_module");
    assert_eq!(
        inventory.directories,
        vec!["services", "services/api", "shared"]
    );
    assert_eq!(
        inventory.modules,
        vec![
            discovery::Module {
                root: "services/api".into(),
                path: "example.com/oracle/api".into(),
            },
            discovery::Module {
                root: "shared".into(),
                path: "example.com/oracle/shared".into(),
            },
        ]
    );
    assert_eq!(
        inventory
            .module_for_file("services/api/main.go")
            .unwrap()
            .path,
        "example.com/oracle/api"
    );
    assert_eq!(
        inventory.module_for_file("shared/helper.go").unwrap().path,
        "example.com/oracle/shared"
    );
}

fn structural_records(records: &[FactRecord]) -> Vec<FactRecord> {
    let ids = records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node)
                if matches!(node.kind.as_str(), "repository" | "directory" | "file") =>
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
                edge.relation == "contains"
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

struct TempDirectory {
    path: PathBuf,
}

impl TempDirectory {
    fn new(name: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lexicon-go-phase3-{name}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

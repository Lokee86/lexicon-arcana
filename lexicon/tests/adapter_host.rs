mod support;

use std::fs;
use std::sync::Arc;

use lexicon::{
    ADAPTER_CONTRACT_VERSION, AdapterContract, AdapterError, AdapterHost, AdapterMode,
    AdapterRequest, Analysis, FactHeader, FactRecord, LanguageAdapter, NodeRecord, SourceSpan,
};

use support::TestDirectory;

struct EchoAdapter;

impl LanguageAdapter for EchoAdapter {
    fn implementation_version(&self) -> &'static str {
        "test"
    }
    fn implementation_fingerprint(&self) -> String {
        "echo".into()
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        Ok(Analysis::new(
            FactHeader {
                adapter_version: "test".into(),
                changed_files: None,
                language: request.language.clone(),
                mode: None,
                record: "lexicon".into(),
                removed_files: None,
                repository: "repo".into(),
                schema_version: lexicon::FACT_SCHEMA_VERSION,
                shared_complete: None,
            },
            Vec::new(),
        ))
    }
}

struct UnsortedAdapter;

impl LanguageAdapter for UnsortedAdapter {
    fn implementation_version(&self) -> &'static str {
        "test"
    }
    fn implementation_fingerprint(&self) -> String {
        "unsorted".into()
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        let high = format!("sha256:{}", "f".repeat(64));
        let low = format!("sha256:{}", "0".repeat(64));
        Ok(Analysis::new(
            FactHeader {
                adapter_version: "test".into(),
                changed_files: None,
                language: request.language.clone(),
                mode: None,
                record: "lexicon".into(),
                removed_files: None,
                repository: "repo".into(),
                schema_version: lexicon::FACT_SCHEMA_VERSION,
                shared_complete: None,
            },
            vec![
                FactRecord::Node(NodeRecord {
                    attributes: None,
                    content_id: None,
                    id: high,
                    kind: "module".into(),
                    name: "high".into(),
                    owner: None,
                    path: "b.py".into(),
                    qualified_name: "high".into(),
                    span: None,
                }),
                FactRecord::Node(NodeRecord {
                    attributes: None,
                    content_id: None,
                    id: low,
                    kind: "module".into(),
                    name: "low".into(),
                    owner: None,
                    path: "a.py".into(),
                    qualified_name: "low".into(),
                    span: None,
                }),
            ],
        ))
    }
}

struct IncrementalOverproduceAdapter;

impl LanguageAdapter for IncrementalOverproduceAdapter {
    fn implementation_version(&self) -> &'static str {
        "test"
    }
    fn implementation_fingerprint(&self) -> String {
        "incremental-overproduce".into()
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        let node = |id: &str, path: &str| {
            FactRecord::Node(NodeRecord {
                attributes: None,
                content_id: None,
                id: format!("sha256:{id}"),
                kind: "function".into(),
                name: path.into(),
                owner: None,
                path: path.into(),
                qualified_name: path.into(),
                span: Some(SourceSpan {
                    end_column: 2,
                    end_line: 1,
                    path: path.into(),
                    start_column: 1,
                    start_line: 1,
                }),
            })
        };
        Ok(Analysis::new(
            FactHeader {
                adapter_version: "test".into(),
                changed_files: Some(request.changed_files.clone()),
                language: request.language.clone(),
                mode: Some("incremental".into()),
                record: "lexicon".into(),
                removed_files: Some(request.removed_files.clone()),
                repository: "repo".into(),
                schema_version: lexicon::FACT_SCHEMA_VERSION,
                shared_complete: Some(true),
            },
            vec![
                node(&"1".repeat(64), "changed.py"),
                node(&"2".repeat(64), "context.py"),
                FactRecord::Node(NodeRecord {
                    attributes: None,
                    content_id: None,
                    id: format!("sha256:{}", "3".repeat(64)),
                    kind: "module".into(),
                    name: "shared".into(),
                    owner: None,
                    path: "context.py".into(),
                    qualified_name: "shared".into(),
                    span: None,
                }),
            ],
        ))
    }
}

struct FutureAdapter;

impl LanguageAdapter for FutureAdapter {
    fn implementation_version(&self) -> &'static str {
        "test"
    }
    fn implementation_fingerprint(&self) -> String {
        "future".into()
    }

    fn contract(&self) -> AdapterContract {
        AdapterContract {
            version: ADAPTER_CONTRACT_VERSION + 1,
            fact_schema_version: lexicon::FACT_SCHEMA_VERSION,
        }
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        EchoAdapter.analyze(request)
    }
}

#[test]
fn registered_adapter_returns_typed_analysis_directly() {
    let root = TestDirectory::new("native-adapter");
    let mut host = AdapterHost::new(root.path.join("adapters"));
    host.register("python", Arc::new(EchoAdapter));

    let request = AdapterRequest {
        language: "python".into(),
        repository: root.path.join("repo"),
        ..Default::default()
    };
    let analysis = host.analyze(&request).unwrap();
    assert_eq!(analysis.header.language, "python");
    assert!(analysis.records.is_empty());
}

#[test]
fn host_canonicalizes_typed_facts_before_validation() {
    let root = TestDirectory::new("adapter-canonicalize");
    let mut host = AdapterHost::new(root.path.join("adapters"));
    host.register("python", Arc::new(UnsortedAdapter));
    let analysis = host
        .analyze(&AdapterRequest {
            language: "python".into(),
            repository: root.path.join("repo"),
            ..Default::default()
        })
        .unwrap();
    let ids = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) => Some(node.id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(ids[0] < ids[1]);
}

#[test]
fn host_filters_incremental_context_ownership_in_base_lexicon() {
    let root = TestDirectory::new("adapter-incremental-filter");
    let mut host = AdapterHost::new(root.path.join("adapters"));
    host.register("python", Arc::new(IncrementalOverproduceAdapter));
    let analysis = host
        .analyze(&AdapterRequest {
            language: "python".into(),
            mode: AdapterMode::Incremental,
            repository: root.path.join("repo"),
            changed_files: vec!["changed.py".into()],
            ..Default::default()
        })
        .unwrap();

    let names = analysis
        .records
        .iter()
        .filter_map(|record| match record {
            FactRecord::Node(node) => Some(node.name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(names.contains(&"changed.py"));
    assert!(names.contains(&"shared"));
    assert!(!names.contains(&"context.py"));
}

#[test]
fn host_rejects_missing_and_unsupported_contracts() {
    let root = TestDirectory::new("adapter-contract");
    let mut host = AdapterHost::new(root.path.join("adapters"));

    let request = AdapterRequest {
        language: "java".into(),
        repository: root.path.join("repo"),
        ..Default::default()
    };
    assert!(
        host.analyze(&request)
            .unwrap_err()
            .to_string()
            .contains("no adapter registered")
    );

    host.register("java", Arc::new(FutureAdapter));
    assert!(
        host.analyze(&request)
            .unwrap_err()
            .to_string()
            .contains("contract version")
    );
}

#[test]
fn python_is_registered_by_default_and_fingerprint_ignores_legacy_adapter_files() {
    let root = TestDirectory::new("native-adapter-fingerprint");
    let adapter_root = root.path.join("adapters");
    let host = AdapterHost::new(&adapter_root);

    assert!(host.has_adapter("python"));
    let first = host.fingerprint("python").unwrap();

    write(&adapter_root, "python/legacy.py", "old = 1\n");
    write(&adapter_root, "python/nested/legacy.py", "old = 2\n");
    assert_eq!(host.fingerprint("python").unwrap(), first);
}

#[test]
fn fingerprint_tracks_registered_native_implementation_and_rejects_missing_adapters() {
    let root = TestDirectory::new("native-fingerprint-identity");
    let mut host = AdapterHost::new(root.path.join("adapters"));

    host.register("ruby", Arc::new(EchoAdapter));
    let first = host.fingerprint("ruby").unwrap();
    host.register("ruby", Arc::new(UnsortedAdapter));
    let second = host.fingerprint("ruby").unwrap();

    assert_ne!(first, second);
    assert!(host.fingerprint("go").is_err());
}

fn write(root: &std::path::Path, relative: &str, content: &str) {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

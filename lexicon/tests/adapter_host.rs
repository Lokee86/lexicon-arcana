mod support;

use std::fs;
use std::sync::Arc;

use lexicon::{
    ADAPTER_CONTRACT_VERSION, AdapterContract, AdapterError, AdapterHost, AdapterRequest, Analysis,
    FactHeader, FactRecord, LanguageAdapter, NodeRecord, adapter_fingerprint,
    adapter_fingerprint_with_versions,
};

use support::TestDirectory;

struct EchoAdapter;

impl LanguageAdapter for EchoAdapter {
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

struct FutureAdapter;

impl LanguageAdapter for FutureAdapter {
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
fn host_rejects_missing_and_unsupported_contracts() {
    let root = TestDirectory::new("adapter-contract");
    let mut host = AdapterHost::new(root.path.join("adapters"));

    let request = AdapterRequest {
        language: "python".into(),
        repository: root.path.join("repo"),
        ..Default::default()
    };
    assert!(
        host.analyze(&request)
            .unwrap_err()
            .to_string()
            .contains("no adapter registered")
    );

    host.register("python", Arc::new(FutureAdapter));
    assert!(
        host.analyze(&request)
            .unwrap_err()
            .to_string()
            .contains("contract version")
    );
}

#[test]
fn fingerprint_matches_go_oracle_and_ignores_test_state_files() {
    let root = TestDirectory::new("adapter-fingerprint");
    write(&root.path, "python/z.py", "z = 1\n");
    write(&root.path, "python/nested/a.py", "a = 1\n");
    write(&root.path, "python/tests/test_adapter.py", "ignored = 1\n");
    write(&root.path, "python/.git/generated.txt", "ignored\n");

    let first = adapter_fingerprint_with_versions(&root.path, "python", 1, 1).unwrap();
    assert_eq!(
        first,
        "sha256:65691dba13735e6d977fbab119af6160c7726bad76bd4f5a02f2b442ac8a1444"
    );
    assert_eq!(adapter_fingerprint(&root.path, "python").unwrap(), first);

    write(
        &root.path,
        "python/tests/test_adapter.py",
        "changed but ignored\n",
    );
    write(
        &root.path,
        "python/.git/generated.txt",
        "changed but ignored\n",
    );
    assert_eq!(adapter_fingerprint(&root.path, "python").unwrap(), first);

    write(&root.path, "python/z.py", "z = 2\n");
    assert_ne!(adapter_fingerprint(&root.path, "python").unwrap(), first);
}

#[test]
fn fingerprint_includes_contract_and_config_versions_and_rejects_missing_adapters() {
    let root = TestDirectory::new("adapter-fingerprint-version");
    write(&root.path, "ruby/adapter.rb", "puts 'ok'\n");

    let base = adapter_fingerprint_with_versions(&root.path, "ruby", 1, 1).unwrap();
    assert_ne!(
        adapter_fingerprint_with_versions(&root.path, "ruby", 2, 1).unwrap(),
        base
    );
    assert_ne!(
        adapter_fingerprint_with_versions(&root.path, "ruby", 1, 2).unwrap(),
        base
    );
    assert!(adapter_fingerprint(&root.path, "unknown-language").is_err());
    assert!(adapter_fingerprint(&root.path, "go").is_err());
}

fn write(root: &std::path::Path, relative: &str, content: &str) {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

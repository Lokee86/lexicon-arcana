mod support;

use std::sync::Arc;

use lexicon::{
    AdapterError, AdapterHost, AdapterMode, AdapterRequest, Analysis, EdgeRecord, FactHeader,
    FactRecord, LanguageAdapter, NodeRecord, Store, execute_analysis_plans,
};

use support::TestDirectory;
use support::scan_adapter::{
    FixtureAdapter, empty_manifest, file_object, full_plan, incremental_plan, paths, write,
};

#[test]
fn full_then_incremental_execution_materializes_and_reuses_objects() {
    let root = TestDirectory::new("scan-execute");
    let source = root.path.join("source");
    let temporary = root.path.join("tmp");
    let store = Store::new(root.path.join("store"));
    let adapter_root = root.path.join("adapters");
    write(&source, "a.py", "value = 1\n");
    write(&source, "pyproject.toml", "[project]\nname='sample'\n");
    write(&adapter_root, "python/adapter.py", "version = 1\n");

    let adapter = Arc::new(FixtureAdapter::new(false));
    let mut host = AdapterHost::new(&adapter_root);
    host.register("python", adapter.clone());

    let full = execute_analysis_plans(
        &store,
        &host,
        &source,
        &temporary,
        empty_manifest(),
        &[full_plan()],
    )
    .unwrap();
    assert!(!temporary.join("python.jsonl").exists());
    let python = full.language("python").unwrap();
    assert_eq!(
        paths(python),
        vec!["a.py".to_owned(), "pyproject.toml".to_owned()]
    );
    let before_config = file_object(python, "pyproject.toml");
    let mut published = full;
    published.state_commit = "state-1".into();
    store.publish(&published).unwrap();

    write(&source, "a.py", "value = 2\n");
    let incremental = execute_analysis_plans(
        &store,
        &host,
        &source,
        &temporary,
        published.clone(),
        &[incremental_plan()],
    )
    .unwrap();
    assert!(!temporary.join("python.jsonl").exists());
    let python = incremental.language("python").unwrap();
    assert_eq!(file_object(python, "pyproject.toml"), before_config);
    assert_ne!(
        file_object(python, "a.py"),
        file_object(published.language("python").unwrap(), "a.py")
    );
    assert_eq!(*adapter.requests.lock().unwrap(), vec![false, true]);
}

#[test]
fn incremental_execution_drops_stale_changed_edges_and_reuses_context() {
    let root = TestDirectory::new("scan-execute-ownership");
    let source = root.path.join("source");
    let temporary = root.path.join("tmp");
    let store = Store::new(root.path.join("store"));
    let adapter_root = root.path.join("adapters");
    write(&source, "a.py", "value = 1\n");
    write(&source, "b.py", "value = 2\n");

    let mut host = AdapterHost::new(&adapter_root);
    host.register("python", Arc::new(OwnershipAdapter));

    let full = execute_analysis_plans(
        &store,
        &host,
        &source,
        &temporary,
        empty_manifest(),
        &[full_plan()],
    )
    .unwrap();
    let before = full.language("python").unwrap();
    let before_a = file_object(before, "a.py");
    let before_b = file_object(before, "b.py");
    let before_shared = before.shared_object_id.clone();
    assert!(
        store
            .load_object(&before_a)
            .unwrap()
            .records
            .iter()
            .any(|record| matches!(record, FactRecord::Edge(edge) if edge.relation == "calls"))
    );

    let mut published = full;
    published.state_commit = "state-ownership".into();
    store.publish(&published).unwrap();
    write(&source, "a.py", "value = 3\n");

    let incremental = execute_analysis_plans(
        &store,
        &host,
        &source,
        &temporary,
        published,
        &[incremental_plan()],
    )
    .unwrap();
    let after = incremental.language("python").unwrap();
    let after_a = file_object(after, "a.py");
    assert_ne!(after_a, before_a);
    assert_eq!(file_object(after, "b.py"), before_b);
    assert_eq!(after.shared_object_id, before_shared);
    assert!(
        store
            .load_object(&after_a)
            .unwrap()
            .records
            .iter()
            .all(|record| !matches!(record, FactRecord::Edge(edge) if edge.relation == "calls"))
    );
}

struct OwnershipAdapter;

impl LanguageAdapter for OwnershipAdapter {
    fn implementation_version(&self) -> &'static str {
        "ownership-test"
    }

    fn implementation_fingerprint(&self) -> String {
        "ownership-test".into()
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        let incremental = request.mode == AdapterMode::Incremental;
        let a = format!("sha256:{}", "1".repeat(64));
        let b = format!("sha256:{}", "2".repeat(64));
        let shared = format!("sha256:{}", "3".repeat(64));
        let mut records = vec![
            owned_node(&a, "a.py"),
            owned_node(&b, "b.py"),
            FactRecord::Node(NodeRecord {
                attributes: None,
                content_id: None,
                id: shared,
                kind: "module".into(),
                name: "shared".into(),
                owner: None,
                path: ".lexicon-repository".into(),
                qualified_name: "shared".into(),
                span: None,
            }),
        ];
        if !incremental {
            records.push(FactRecord::Edge(EdgeRecord {
                attributes: None,
                owner: Some("a.py".into()),
                relation: "calls".into(),
                source: a,
                span: None,
                target: b,
            }));
        }
        Ok(Analysis::new(
            FactHeader {
                adapter_version: "ownership-test".into(),
                changed_files: incremental.then(|| request.changed_files.clone()),
                language: request.language.clone(),
                mode: Some(if incremental { "incremental" } else { "full" }.into()),
                record: "lexicon".into(),
                removed_files: incremental.then(|| request.removed_files.clone()),
                repository: "repo".into(),
                schema_version: 1,
                shared_complete: incremental.then_some(true),
            },
            records,
        ))
    }
}

fn owned_node(id: &str, path: &str) -> FactRecord {
    FactRecord::Node(NodeRecord {
        attributes: None,
        content_id: None,
        id: id.into(),
        kind: "function".into(),
        name: path.into(),
        owner: Some(path.into()),
        path: path.into(),
        qualified_name: format!("{path}::value"),
        span: None,
    })
}

#[test]
fn scoped_adapter_failure_retries_full_analysis() {
    let root = TestDirectory::new("scan-execute-retry");
    let source = root.path.join("source");
    let store = Store::new(root.path.join("store"));
    let adapter_root = root.path.join("adapters");
    write(&source, "a.py", "value = 1\n");
    write(&adapter_root, "python/adapter.py", "version = 1\n");

    let adapter = Arc::new(FixtureAdapter::new(true));
    let mut host = AdapterHost::new(&adapter_root);
    host.register("python", adapter.clone());

    let initial = execute_analysis_plans(
        &store,
        &host,
        &source,
        &root.path.join("tmp"),
        empty_manifest(),
        &[full_plan()],
    )
    .unwrap();
    let mut published = initial;
    published.state_commit = "state-1".into();
    store.publish(&published).unwrap();

    write(&source, "a.py", "value = 2\n");
    let updated = execute_analysis_plans(
        &store,
        &host,
        &source,
        &root.path.join("tmp"),
        published,
        &[incremental_plan()],
    )
    .unwrap();
    assert!(updated.language("python").is_some());
    assert_eq!(*adapter.requests.lock().unwrap(), vec![false, true, false]);
}

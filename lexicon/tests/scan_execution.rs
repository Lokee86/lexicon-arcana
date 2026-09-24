mod support;

use std::sync::Arc;

use lexicon::{AdapterHost, Store, execute_analysis_plans};

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
    host.register_native("python", adapter.clone());

    let full = execute_analysis_plans(
        &store,
        &host,
        &source,
        &temporary,
        empty_manifest(),
        &[full_plan()],
    )
    .unwrap();
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
    let python = incremental.language("python").unwrap();
    assert_eq!(file_object(python, "pyproject.toml"), before_config);
    assert_ne!(
        file_object(python, "a.py"),
        file_object(published.language("python").unwrap(), "a.py")
    );
    assert_eq!(*adapter.requests.lock().unwrap(), vec![false, true]);
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
    host.register_native("python", adapter.clone());

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

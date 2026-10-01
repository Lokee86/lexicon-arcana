mod support;

use std::fs;

use lexicon::{Analysis, EdgeRecord, FactHeader, FactRecord, SnapshotManifest, SourceFile, Store};
use support::TestDirectory;

fn fixture() -> (Analysis, Vec<SourceFile>) {
    let mut records = Vec::new();
    for file in ["a.py", "b.py", "c.py", "pkg/old.py"] {
        records.push(FactRecord::Node(lexicon::NodeRecord {
            attributes: None,
            content_id: None,
            id: format!("node-{file}"),
            kind: "function".into(),
            name: file.into(),
            owner: Some(file.into()),
            path: file.into(),
            qualified_name: file.into(),
            span: None,
        }));
    }
    for (source, target, owner) in [("node-c.py", "node-b.py", "c.py")] {
        records.push(FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner: Some(owner.into()),
            source: source.into(),
            target: target.into(),
            relation: "calls".into(),
            span: None,
        }));
    }
    records.push(FactRecord::Unresolved(lexicon::UnresolvedRecord {
        attributes: None,
        candidate_name: Some("pkg.new".into()),
        candidate_namespace: None,
        expression: "pkg.new".into(),
        owner: Some("pkg/old.py".into()),
        reason: "missing-target".into(),
        relation: "imports".into(),
        source: "node-pkg/old.py".into(),
        span: None,
    }));
    // This node has no direct owner and is stored in the shared fact object.
    // It overrides any file-node owner when its path belongs to a known file.
    records.push(FactRecord::Node(lexicon::NodeRecord {
        attributes: None,
        content_id: None,
        id: "shared-a".into(),
        kind: "function".into(),
        name: "shared".into(),
        owner: None,
        path: "a.py".into(),
        qualified_name: "shared".into(),
        span: None,
    }));
    records.push(FactRecord::Edge(EdgeRecord {
        attributes: None,
        owner: Some("b.py".into()),
        source: "node-b.py".into(),
        target: "shared-a".into(),
        relation: "references".into(),
        span: None,
    }));
    let files = ["a.py", "b.py", "c.py", "pkg/old.py"]
        .map(|path| SourceFile {
            path: path.into(),
            content: format!("content {path}").into_bytes(),
        })
        .to_vec();
    (
        Analysis::new(
            FactHeader {
                adapter_version: "v1".into(),
                changed_files: None,
                language: "python".into(),
                mode: Some("full".into()),
                record: "lexicon".into(),
                removed_files: None,
                repository: "repo".into(),
                schema_version: 1,
                shared_complete: None,
            },
            records,
        ),
        files,
    )
}

fn publish(store: &Store, state: &str, entry: lexicon::LanguageEntry) -> String {
    store
        .publish(&SnapshotManifest {
            version: 1,
            state_commit: state.into(),
            languages: Some(vec![entry]),
        })
        .unwrap()
}

#[test]
fn full_index_and_legacy_bootstrap_match_scope_and_additions() {
    let directory = TestDirectory::new("dependency-index-parity");
    let store = Store::new(&directory.path);
    let (analysis, sources) = fixture();
    let full = store
        .build_full_language(&analysis, &sources, "python", "cfg", "fp")
        .unwrap();
    assert!(!full.dependency_index_id.is_empty());
    let full_snapshot = publish(&store, "full", full.clone());
    let roots = ["a.py".to_owned()];
    let expected = store.incremental_scope("python", &roots).unwrap();
    assert_eq!(expected.emit, ["a.py", "b.py"]);
    assert_eq!(expected.context, ["a.py", "b.py"]);
    assert!(!expected.full_required);
    // b -> shared-a is the ONLY b -> a edge: shared-node ownership matters.
    let from_b = store.incremental_scope("python", &["b.py".into()]).unwrap();
    assert_eq!(from_b.emit, ["b.py", "c.py"]);
    assert_eq!(from_b.context, ["a.py", "b.py", "c.py"]);
    assert!(
        store
            .incremental_scope("python", &["missing.py".into()])
            .unwrap()
            .full_required
    );
    assert!(
        store
            .incremental_scope_with_additions("python", &roots, &["pkg/new.py".into()])
            .unwrap()
            .full_required
    );
    assert!(
        !store
            .incremental_scope_with_additions("python", &roots, &["pkg/other.py".into()])
            .unwrap()
            .full_required
    );
    let mut legacy = full.clone();
    legacy.dependency_index_id.clear();
    let legacy_snapshot = publish(&store, "legacy", legacy);
    assert_ne!(legacy_snapshot, full_snapshot);
    assert_eq!(store.incremental_scope("python", &roots).unwrap(), expected);
    assert!(
        store
            .incremental_scope_with_additions("python", &roots, &["pkg/new.py".into()])
            .unwrap()
            .full_required
    );
    assert!(
        directory
            .path
            .join("topology/bootstrap")
            .join(legacy_snapshot.trim_start_matches("sha256:"))
            .exists()
    );
}

#[test]
fn indexed_queries_never_load_unrelated_fact_objects() {
    let directory = TestDirectory::new("dependency-index-object-free");
    let store = Store::new(&directory.path);
    let (analysis, sources) = fixture();
    let entry = store
        .build_full_language(&analysis, &sources, "python", "cfg", "fp")
        .unwrap();
    publish(&store, "full", entry);
    fs::rename(
        directory.path.join("objects"),
        directory.path.join("objects-hidden"),
    )
    .unwrap();
    let scope = store.incremental_scope("python", &["a.py".into()]).unwrap();
    assert_eq!(scope.emit, ["a.py", "b.py"]);
    assert_eq!(scope.context, ["a.py", "b.py"]);
}

#[test]
fn bootstrapped_legacy_snapshot_is_indexed_once() {
    let directory = TestDirectory::new("dependency-index-legacy");
    let store = Store::new(&directory.path);
    let (analysis, sources) = fixture();
    let mut entry = store
        .build_full_language(&analysis, &sources, "python", "cfg", "fp")
        .unwrap();
    entry.dependency_index_id.clear();
    let snapshot = publish(&store, "legacy", entry.clone());
    let before = store.incremental_scope("python", &["a.py".into()]).unwrap();
    let pointer = directory
        .path
        .join("topology/bootstrap")
        .join(snapshot.trim_start_matches("sha256:"));
    assert_eq!(fs::read_dir(&pointer).unwrap().count(), 1);
    fs::rename(
        directory.path.join("objects"),
        directory.path.join("objects-hidden"),
    )
    .unwrap();
    assert_eq!(
        store.incremental_scope("python", &["a.py".into()]).unwrap(),
        before
    );
    // The exact snapshot ID matters even when the language entry is identical.
    let next = publish(&store, "legacy-next", entry);
    assert_ne!(next, snapshot);
    assert!(store.incremental_scope("python", &["a.py".into()]).is_err());
}

#[test]
fn publication_rejects_corrupted_referenced_partition() {
    let directory = TestDirectory::new("dependency-index-broken-shard");
    let store = Store::new(&directory.path);
    let (analysis, sources) = fixture();
    let entry = store
        .build_full_language(&analysis, &sources, "python", "cfg", "fp")
        .unwrap();
    let root_id = entry.dependency_index_id.trim_start_matches("sha256:");
    let root_path = directory
        .path
        .join("topology/objects")
        .join(&root_id[..2])
        .join(&root_id[2..]);
    let root: serde_json::Value = serde_json::from_slice(&fs::read(root_path).unwrap()).unwrap();
    let first_shard = root["files"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .as_str()
        .unwrap()
        .trim_start_matches("sha256:");
    let shard_path = directory
        .path
        .join("topology/objects")
        .join(&first_shard[..2])
        .join(&first_shard[2..]);
    fs::write(shard_path, b"broken partition").unwrap();
    assert!(
        store
            .publish(&SnapshotManifest {
                version: 1,
                state_commit: "invalid".into(),
                languages: Some(vec![entry]),
            })
            .is_err()
    );
    assert!(store.current().is_err());
}

#[test]
fn index_corruption_and_wrong_generation_fail_closed() {
    let directory = TestDirectory::new("dependency-index-corrupt");
    let store = Store::new(&directory.path);
    let (analysis, sources) = fixture();
    let entry = store
        .build_full_language(&analysis, &sources, "python", "cfg", "fp")
        .unwrap();
    let mut wrong = entry.clone();
    wrong.analysis_config_id = "different".into();
    assert!(
        store
            .publish(&SnapshotManifest {
                version: 1,
                state_commit: "wrong".into(),
                languages: Some(vec![wrong]),
            })
            .is_err()
    );
    assert!(store.current().is_err());
    publish(&store, "full", entry.clone());
    let hex = entry.dependency_index_id.trim_start_matches("sha256:");
    let root = directory
        .path
        .join("topology/objects")
        .join(&hex[..2])
        .join(&hex[2..]);
    fs::write(root, b"corrupt").unwrap();
    assert!(store.incremental_scope("python", &["a.py".into()]).is_err());
}

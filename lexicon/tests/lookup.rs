use lexicon::{
    EdgeRecord, FactObject, FactRecord, FileEntry, LanguageEntry, LookupDirection, LookupError,
    LookupReference, NodeRecord, SnapshotLookup, SnapshotManifest, Store, UnresolvedRecord,
    content_id, node_id,
};

#[test]
fn finds_resolves_and_reports_direct_snapshot_evidence() {
    let root = TestDirectory::new("lookup");
    let store = Store::new(root.path.join("state"));

    let caller = id("caller");
    let target = id("target");
    let possible = id("possible");
    let other_target = id("other-target");

    let first = write_object(
        &store,
        "a.py",
        vec![
            node(&caller, "caller", "pkg.caller", "a.py"),
            edge(&caller, &target, "calls", "a.py"),
            edge(&caller, &possible, "possible-calls", "a.py"),
            edge(&caller, &possible, "references", "a.py"),
            unresolved(&caller, "calls", "dynamic()", "a.py"),
        ],
    );
    let second = write_object(
        &store,
        "b.py",
        vec![
            node(&target, "target", "pkg.target", "b.py"),
            node(&possible, "choice", "pkg.choice", "b.py"),
            node(&other_target, "target", "other.target", "b.py"),
        ],
    );
    let snapshot = publish(&store, vec![first, second]);
    let lookup = SnapshotLookup::load(&store, "CURRENT").unwrap();

    assert_eq!(lookup.snapshot_id(), snapshot);
    let found = lookup.find("pkg.target", 10);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].node.id, target);
    assert_eq!(lookup.resolve("pkg.caller").unwrap().node.id, caller);

    match lookup.resolve("target").unwrap_err() {
        LookupError::Ambiguous { candidates, .. } => assert_eq!(candidates.len(), 2),
        error => panic!("unexpected error: {error}"),
    }

    let refs = lookup.refs(&caller, 10).unwrap();
    assert_eq!(refs.len(), 4);
    assert!(refs.iter().any(|value| matches!(
        value,
        LookupReference::Edge(edge)
            if edge.direction == LookupDirection::Outgoing
                && edge.edge.relation == "references"
    )));

    let calls = lookup.calls(&caller, 10).unwrap();
    assert_eq!(calls.len(), 3);
    assert!(calls.iter().any(|value| matches!(
        value,
        LookupReference::Edge(edge) if edge.edge.relation == "calls"
    )));
    assert!(calls.iter().any(|value| matches!(
        value,
        LookupReference::Edge(edge) if edge.edge.relation == "possible-calls"
    )));
    assert!(calls.iter().any(|value| matches!(
        value,
        LookupReference::Unresolved(value)
            if value.record.relation == "calls" && value.record.expression == "dynamic()"
    )));
    assert!(!calls.iter().any(|value| matches!(
        value,
        LookupReference::Edge(edge) if edge.edge.relation == "references"
    )));

    let incoming = lookup.calls(&target, 10).unwrap();
    assert!(matches!(
        &incoming[0],
        LookupReference::Edge(edge) if edge.direction == LookupDirection::Incoming
    ));
}

#[test]
fn find_is_ranked_case_insensitively_and_bounded() {
    let root = TestDirectory::new("lookup-find");
    let store = Store::new(root.path.join("state"));
    let file = write_object(
        &store,
        "main.py",
        vec![
            node(&id("alpha"), "Alpha", "pkg.Alpha", "main.py"),
            node(&id("alphabet"), "Alphabet", "pkg.Alphabet", "main.py"),
            node(&id("other"), "other", "pkg.alpha_helper", "main.py"),
        ],
    );
    publish(&store, vec![file]);

    let lookup = SnapshotLookup::load(&store, "CURRENT").unwrap();
    let found = lookup.find("alpha", 2);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].node.name, "Alpha");
    assert_eq!(found[1].node.name, "Alphabet");
    assert!(lookup.find("", 10).is_empty());
    assert!(lookup.find("alpha", 0).is_empty());
}

fn id(name: &str) -> String {
    node_id("python", "function", name)
}

fn node(id: &str, name: &str, qualified_name: &str, path: &str) -> FactRecord {
    FactRecord::Node(NodeRecord {
        attributes: None,
        content_id: None,
        id: id.into(),
        kind: "function".into(),
        name: name.into(),
        owner: Some(path.into()),
        path: path.into(),
        qualified_name: qualified_name.into(),
        span: None,
    })
}

fn edge(source: &str, target: &str, relation: &str, path: &str) -> FactRecord {
    FactRecord::Edge(EdgeRecord {
        attributes: None,
        owner: Some(path.into()),
        relation: relation.into(),
        source: source.into(),
        span: None,
        target: target.into(),
    })
}

fn unresolved(source: &str, relation: &str, expression: &str, path: &str) -> FactRecord {
    FactRecord::Unresolved(UnresolvedRecord {
        attributes: None,
        candidate_name: Some("dynamic".into()),
        candidate_namespace: None,
        expression: expression.into(),
        owner: Some(path.into()),
        reason: "dynamic-target".into(),
        relation: relation.into(),
        source: source.into(),
        span: None,
    })
}

fn write_object(store: &Store, path: &str, records: Vec<FactRecord>) -> FileEntry {
    let source_content_id = content_id(path.as_bytes());
    let object = FactObject {
        version: 1,
        language: "python".into(),
        owner: path.into(),
        source_content_id: source_content_id.clone(),
        adapter_version: "test".into(),
        schema_version: 1,
        analysis_config_id: "config".into(),
        records,
    };
    let object_id = store.write_object(&object).unwrap();
    FileEntry {
        path: path.into(),
        language: "python".into(),
        content_id: source_content_id,
        object_id,
    }
}

fn publish(store: &Store, files: Vec<FileEntry>) -> String {
    store
        .publish(&SnapshotManifest {
            version: 1,
            state_commit: "state".into(),
            languages: Some(vec![LanguageEntry {
                language: "python".into(),
                adapter_version: "test".into(),
                adapter_fingerprint: String::new(),
                schema_version: 1,
                repository: "repo".into(),
                analysis_config_id: "config".into(),
                shared_object_id: String::new(),
                dependency_index_id: String::new(),
                files: Some(files),
            }]),
        })
        .unwrap()
}

struct TestDirectory {
    path: std::path::PathBuf,
}

impl TestDirectory {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("lexicon-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

use lexicon::{
    Analysis, EdgeRecord, FactHeader, FactRecord, NodeRecord, SourceFile, Store, UnresolvedRecord,
};

mod support;
use support::TestDirectory;

fn node(id: &str, kind: &str, path: &str, owner: Option<&str>) -> FactRecord {
    FactRecord::Node(NodeRecord {
        attributes: None,
        content_id: None,
        id: id.into(),
        kind: kind.into(),
        name: id.into(),
        owner: owner.map(str::to_owned),
        path: path.into(),
        qualified_name: id.into(),
        span: None,
    })
}
fn header(mode: &str) -> FactHeader {
    FactHeader {
        adapter_version: "test".into(),
        changed_files: (mode == "incremental").then(|| vec!["a.py".into()]),
        language: "python".into(),
        mode: Some(mode.into()),
        record: "lexicon".into(),
        removed_files: (mode == "incremental").then(Vec::new),
        repository: "repo".into(),
        schema_version: 1,
        shared_complete: Some(true),
    }
}
fn sources() -> Vec<SourceFile> {
    vec![
        SourceFile {
            path: "a.py".into(),
            content: b"value = 1\n".to_vec(),
        },
        SourceFile {
            path: "b.py".into(),
            content: b"from a import value\n".to_vec(),
        },
    ]
}
fn edge() -> FactRecord {
    FactRecord::Edge(EdgeRecord {
        attributes: Some(serde_json::json!({"source":"b","category":"local"})),
        owner: None,
        relation: "depends-on".into(),
        source: "module-b".into(),
        target: "module-a".into(),
        span: None,
    })
}
fn alternate_edge() -> FactRecord {
    let mut value = edge();
    if let FactRecord::Edge(edge) = &mut value {
        edge.attributes = Some(serde_json::json!({"source":"b.alternate","category":"local"}));
    }
    value
}
fn unresolved(candidate: &str) -> FactRecord {
    FactRecord::Unresolved(UnresolvedRecord {
        attributes: None,
        owner: None,
        span: None,
        source: "module-b".into(),
        relation: "imports".into(),
        expression: "optional_module".into(),
        reason: "missing-target".into(),
        candidate_name: Some(candidate.into()),
        candidate_namespace: None,
    })
}
fn initial() -> Vec<FactRecord> {
    vec![
        node("file-a", "file", "a.py", Some("a.py")),
        node("file-b", "file", "b.py", Some("b.py")),
        node("module-a", "module", "a.py", None),
        node("module-b", "module", "b.py", None),
        edge(),
        alternate_edge(),
        unresolved("candidate_x"),
        unresolved("candidate_y"),
    ]
}
fn init(store: &Store) -> lexicon::LanguageEntry {
    store
        .build_full_language(
            &Analysis::new(header("full"), initial()),
            &sources(),
            "python",
            "cfg",
            "fp",
        )
        .unwrap()
}
fn scoped(module: FactRecord) -> Analysis {
    // The scoped adapter has no b.py and cannot re-emit b -> a.
    Analysis::new(
        header("incremental"),
        vec![node("file-a", "file", "a.py", Some("a.py")), module],
    )
}

#[test]
fn stable_target_keeps_untouched_cross_file_shared_edge() {
    let dir = TestDirectory::new("shared-stable-target");
    let store = Store::new(&dir.path);
    let previous = init(&store);
    let updated = store
        .build_incremental_language(
            &previous,
            &scoped(node("module-a", "module", "a.py", None)),
            &[SourceFile {
                path: "a.py".into(),
                content: b"value = 1\n# comment\n".to_vec(),
            }],
            "cfg",
            "fp",
            &["a.py".into()],
            &[],
            true,
        )
        .unwrap();
    assert_eq!(updated.shared_object_id, previous.shared_object_id);
    assert!(!updated.dependency_index_id.is_empty());
    assert!(
        store
            .load_object(&updated.shared_object_id)
            .unwrap()
            .records
            .contains(&edge())
    );
    let shared = store.load_object(&updated.shared_object_id).unwrap();
    assert!(shared.records.contains(&alternate_edge()));
    assert!(shared.records.contains(&unresolved("candidate_x")));
    assert!(shared.records.contains(&unresolved("candidate_y")));
}

#[test]
fn removed_or_changed_target_requires_full_analysis() {
    for replacement in [None, Some(node("module-a", "module", "a.py", None))] {
        let dir = TestDirectory::new("shared-unstable-target");
        let store = Store::new(&dir.path);
        let previous = init(&store);
        let replacement = replacement.map(|mut record| {
            if let FactRecord::Node(node) = &mut record {
                node.attributes = Some(serde_json::json!({"different":true}));
            }
            record
        });
        let result = store.build_incremental_language(
            &previous,
            &scoped(replacement.unwrap_or_else(|| node("new-module", "module", "a.py", None))),
            &[SourceFile {
                path: "a.py".into(),
                content: b"changed\n".to_vec(),
            }],
            "cfg",
            "fp",
            &["a.py".into()],
            &[],
            true,
        );
        assert!(
            result.is_err(),
            "unstable shared target must trigger safe full retry"
        );
    }
}

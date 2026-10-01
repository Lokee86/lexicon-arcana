use std::collections::BTreeMap;

use lexicon::{
    Analysis, EdgeRecord, FactHeader, FactRecord, NodeRecord, SnapshotManifest, SourceFile, Store,
    UnresolvedRecord,
};
pub(super) fn node(id: &str, owner: Option<&str>, path: &str) -> FactRecord {
    FactRecord::Node(NodeRecord {
        attributes: None,
        content_id: None,
        id: id.into(),
        kind: "function".into(),
        name: id.into(),
        owner: owner.map(str::to_owned),
        path: path.into(),
        qualified_name: id.into(),
        span: None,
    })
}
pub(super) fn edge(source: &str, target: &str, owner: &str) -> FactRecord {
    FactRecord::Edge(EdgeRecord {
        attributes: None,
        owner: Some(owner.into()),
        relation: "references".into(),
        source: source.into(),
        target: target.into(),
        span: None,
    })
}
pub(super) fn unresolved(owner: &str, name: &str) -> FactRecord {
    FactRecord::Unresolved(UnresolvedRecord {
        attributes: None,
        candidate_name: Some(name.into()),
        candidate_namespace: None,
        expression: name.into(),
        owner: Some(owner.into()),
        reason: "missing-target".into(),
        relation: "imports".into(),
        source: format!("node-{owner}"),
        span: None,
    })
}
pub(super) fn header(mode: &str, changed: &[&str], removed: &[&str]) -> FactHeader {
    FactHeader {
        adapter_version: "test".into(),
        changed_files: Some(changed.iter().map(|s| (*s).into()).collect()),
        language: "python".into(),
        mode: Some(mode.into()),
        record: "lexicon".into(),
        removed_files: Some(removed.iter().map(|s| (*s).into()).collect()),
        repository: "repo".into(),
        schema_version: 1,
        shared_complete: Some(false),
    }
}
pub(super) struct State {
    pub(super) files: BTreeMap<String, Vec<FactRecord>>,
    pub(super) revisions: BTreeMap<String, u32>,
}
impl State {
    pub(super) fn initial() -> Self {
        Self {
            files: BTreeMap::from([
                ("a.py".into(), vec![node("node-a", Some("a.py"), "a.py")]),
                (
                    "b.py".into(),
                    vec![
                        node("node-b", Some("b.py"), "b.py"),
                        edge("node-b", "shared-a", "b.py"),
                    ],
                ),
                (
                    "c.py".into(),
                    vec![
                        node("node-c", Some("c.py"), "c.py"),
                        edge("node-c", "node-b", "c.py"),
                        edge("node-c", "shared-e", "c.py"),
                    ],
                ),
                (
                    "pkg/old.py".into(),
                    vec![
                        node("node-pkg/old.py", Some("pkg/old.py"), "pkg/old.py"),
                        unresolved("pkg/old.py", "pkg.new"),
                    ],
                ),
            ]),
            revisions: ["a.py", "b.py", "c.py", "pkg/old.py"]
                .iter()
                .map(|path| ((*path).into(), 1))
                .collect(),
        }
    }
    fn shared() -> Vec<FactRecord> {
        vec![
            node("shared-a", None, "a.py"),
            node("shared-e", None, "e.py"),
        ]
    }
    pub(super) fn all_records(&self) -> Vec<FactRecord> {
        self.files
            .values()
            .flat_map(|r| r.clone())
            .chain(Self::shared())
            .collect()
    }
    pub(super) fn sources(&self) -> Vec<SourceFile> {
        self.revisions
            .iter()
            .map(|(path, revision)| SourceFile {
                path: path.clone(),
                content: format!("{path} rev {revision}").into_bytes(),
            })
            .collect()
    }
    pub(super) fn changed(&self, path: &str) -> Vec<SourceFile> {
        self.sources()
            .into_iter()
            .filter(|s| s.path == path)
            .collect()
    }
}
pub(super) fn publish(store: &Store, entry: lexicon::LanguageEntry, step: usize) {
    store
        .publish(&SnapshotManifest {
            version: 1,
            state_commit: format!("state-{step}"),
            languages: Some(vec![entry]),
        })
        .unwrap();
}
pub(super) fn check_parity(indexed: &Store, oracle: &Store, expected_id: &str) {
    let (_, left) = indexed.current().unwrap();
    let (_, right) = oracle.current().unwrap();
    let a = left.language("python").unwrap();
    let b = right.language("python").unwrap();
    assert_eq!(a.files, b.files);
    assert_eq!(a.shared_object_id, b.shared_object_id);
    assert!(!a.dependency_index_id.is_empty());
    assert_eq!(
        a.dependency_index_id, b.dependency_index_id,
        "{expected_id}: index differs"
    );
    for root in ["a.py", "b.py", "c.py", "e.py", "pkg/old.py", "missing.py"] {
        let paths = [root.into()];
        assert_eq!(
            indexed.incremental_scope("python", &paths).unwrap(),
            oracle.incremental_scope("python", &paths).unwrap(),
            "{expected_id}: {root}"
        );
    }
    for path in ["pkg/new.py", "pkg/other.py", "unexpected.txt"] {
        let additions = [path.into()];
        assert_eq!(
            indexed
                .incremental_scope_with_additions("python", &["c.py".into()], &additions)
                .unwrap(),
            oracle
                .incremental_scope_with_additions("python", &["c.py".into()], &additions)
                .unwrap(),
            "{expected_id}: addition {path}",
        );
    }
}

pub(super) fn apply_step(
    indexed: &Store,
    oracle: &Store,
    state: &State,
    step: usize,
    changed: &[&str],
    removed: &[&str],
    label: &str,
) {
    let (_, prior) = indexed.current().unwrap();
    let analysis = Analysis::new(
        header("incremental", changed, removed),
        changed
            .iter()
            .flat_map(|path| state.files.get(*path).cloned().unwrap_or_default())
            .collect(),
    );
    let new_entry = indexed
        .build_incremental_language(
            prior.language("python").unwrap(),
            &analysis,
            &changed
                .iter()
                .flat_map(|path| state.changed(path))
                .collect::<Vec<_>>(),
            "cfg",
            "fp",
            &changed
                .iter()
                .map(|path| (*path).into())
                .collect::<Vec<_>>(),
            &removed
                .iter()
                .map(|path| (*path).into())
                .collect::<Vec<_>>(),
            false,
        )
        .unwrap();
    publish(indexed, new_entry, step);
    let full = Analysis::new(header("full", &[], &[]), state.all_records());
    let oracle_entry = oracle
        .build_full_language(&full, &state.sources(), "python", "cfg", "fp")
        .unwrap();
    publish(oracle, oracle_entry, step);
    check_parity(indexed, oracle, label);
}

mod support;

use lexicon::{
    Analysis, EdgeRecord, FactHeader, FactRecord, NodeRecord, SnapshotManifest, SourceFile, Store,
};
use support::TestDirectory;

#[test]
fn parallel_index_writes_produce_identical_roots_and_snapshots() {
    let left_dir = TestDirectory::new("parallel-cas-left");
    let right_dir = TestDirectory::new("parallel-cas-right");
    let left = Store::new(&left_dir.path);
    let right = Store::new(&right_dir.path);
    let records = vec![
        FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: None,
            id: "one".into(),
            kind: "function".into(),
            name: "one".into(),
            owner: Some("a.py".into()),
            path: "a.py".into(),
            qualified_name: "one".into(),
            span: None,
        }),
        FactRecord::Node(NodeRecord {
            attributes: None,
            content_id: None,
            id: "shared".into(),
            kind: "function".into(),
            name: "shared".into(),
            owner: None,
            path: "b.py".into(),
            qualified_name: "shared".into(),
            span: None,
        }),
        FactRecord::Edge(EdgeRecord {
            attributes: None,
            owner: Some("a.py".into()),
            relation: "calls".into(),
            source: "one".into(),
            target: "shared".into(),
            span: None,
        }),
    ];
    let analysis = Analysis::new(
        FactHeader {
            adapter_version: "test".into(),
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
    );
    let sources = ["a.py", "b.py"]
        .into_iter()
        .map(|path| SourceFile {
            path: path.into(),
            content: format!("content:{path}").into_bytes(),
        })
        .collect::<Vec<_>>();
    let first = left
        .build_full_language(&analysis, &sources, "python", "cfg", "fp")
        .unwrap();
    let second = right
        .build_full_language(&analysis, &sources, "python", "cfg", "fp")
        .unwrap();
    assert_eq!(first, second, "parallel CAS writes must be deterministic");
    let snapshots = [(&left, first), (&right, second)]
        .into_iter()
        .map(|(store, entry)| {
            store
                .publish(&SnapshotManifest {
                    version: 1,
                    state_commit: "same".into(),
                    languages: Some(vec![entry]),
                })
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(snapshots[0], snapshots[1]);
    assert_eq!(
        left.incremental_scope("python", &["b.py".into()]).unwrap(),
        right.incremental_scope("python", &["b.py".into()]).unwrap(),
    );
}

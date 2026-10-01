#![allow(dead_code)]

use lexicon::{
    Analysis, EdgeRecord, FactHeader, FactObject, FactRecord, FileEntry, LanguageEntry, NodeRecord,
    SnapshotManifest, Store, UnresolvedRecord,
};

pub fn write_file(store: &Store, path: &str, records: Vec<FactRecord>) -> FileEntry {
    let object = FactObject {
        version: 1,
        language: "python".into(),
        owner: path.into(),
        source_content_id: format!("content:{path}"),
        adapter_version: "test".into(),
        schema_version: 1,
        analysis_config_id: "config".into(),
        records,
    };
    let object_id = store.write_object(&object).unwrap();
    FileEntry {
        path: path.into(),
        language: "python".into(),
        content_id: object.source_content_id,
        object_id,
    }
}

pub fn publish_language(store: &Store, files: Vec<FileEntry>) {
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
        .unwrap();
}

pub fn node(id: &str, owner: &str) -> FactRecord {
    FactRecord::Node(NodeRecord {
        attributes: None,
        content_id: None,
        id: id.into(),
        kind: "function".into(),
        name: id.into(),
        owner: Some(owner.into()),
        path: owner.into(),
        qualified_name: id.into(),
        span: None,
    })
}

pub fn edge(source: &str, target: &str, relation: &str, owner: &str) -> FactRecord {
    FactRecord::Edge(EdgeRecord {
        attributes: None,
        owner: Some(owner.into()),
        relation: relation.into(),
        source: source.into(),
        span: None,
        target: target.into(),
    })
}

pub fn unresolved(source: &str, reason: &str, owner: &str) -> FactRecord {
    FactRecord::Unresolved(UnresolvedRecord {
        attributes: None,
        candidate_name: None,
        candidate_namespace: None,
        expression: "missing()".into(),
        owner: Some(owner.into()),
        reason: reason.into(),
        relation: "calls".into(),
        source: source.into(),
        span: None,
    })
}

pub fn incremental_analysis(target: &str) -> Analysis {
    Analysis {
        header: FactHeader {
            adapter_version: "test".into(),
            changed_files: Some(vec!["a.py".into()]),
            language: "python".into(),
            mode: Some("incremental".into()),
            record: "lexicon".into(),
            removed_files: Some(Vec::new()),
            repository: "repo".into(),
            schema_version: 1,
            shared_complete: Some(false),
        },
        records: vec![
            node("node-a", "a.py"),
            edge("node-a", target, "calls", "a.py"),
        ],
    }
}

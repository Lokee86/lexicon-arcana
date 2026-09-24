mod support;

use std::fs;

use lexicon::{
    EdgeRecord, FactObject, FactRecord, FileEntry, LanguageEntry, NodeRecord, SnapshotManifest,
    Store, UnresolvedRecord, content_id,
};

use support::TestDirectory;

#[test]
fn export_current_reconstructs_sorted_standalone_library() {
    let fixture = ExportFixture::new("export-current");
    let (store, entry, snapshot) = fixture.store();

    let destination = fixture.root.path.join("export");
    store
        .export("CURRENT", &destination, &["python".into()])
        .unwrap();

    let data = fs::read(destination.join("python.jsonl")).unwrap();
    let text = String::from_utf8(data.clone()).unwrap();
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 6);
    assert_eq!(
        lines[0],
        r#"{"adapter_version":"adapter-1","language":"python","mode":"full","record":"lexicon","repository":"repo","schema_version":1}"#
    );
    assert!(lines[1].contains(r#""id":"node-a""#));
    assert!(lines[2].contains(r#""id":"node-z""#));
    assert!(lines[3].contains(r#""record":"edge""#));
    assert!(lines[4].contains(r#""reason":"shared-record""#));
    assert!(lines[5].contains(r#""reason":"file-record""#));

    let second = fixture.root.path.join("export-again");
    store.export(&snapshot, &second, &[]).unwrap();
    assert_eq!(data, fs::read(second.join("python.jsonl")).unwrap());
    assert!(!entry.shared_object_id.is_empty());
}

#[test]
fn export_rejects_unknown_language_before_publishing() {
    let fixture = ExportFixture::new("export-unknown");
    let (store, _, _) = fixture.store();
    let destination = fixture.root.path.join("export");
    fs::create_dir_all(&destination).unwrap();
    let path = destination.join("python.jsonl");
    fs::write(&path, b"old content\n").unwrap();

    assert!(
        store
            .export("CURRENT", &destination, &["ruby".into()])
            .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), b"old content\n");
}

#[test]
fn export_verifies_all_objects_before_atomic_publish() {
    let fixture = ExportFixture::new("export-corrupt");
    let (store, entry, _) = fixture.store();
    let destination = fixture.root.path.join("export");
    fs::create_dir_all(&destination).unwrap();
    let path = destination.join("python.jsonl");
    fs::write(&path, b"previous complete library\n").unwrap();

    fs::write(
        store.object_path(&entry.files.as_ref().unwrap()[0].object_id),
        b"corrupt\n",
    )
    .unwrap();

    assert!(store.export("CURRENT", &destination, &[]).is_err());
    assert_eq!(fs::read(path).unwrap(), b"previous complete library\n");
}

struct ExportFixture {
    root: TestDirectory,
}

impl ExportFixture {
    fn new(name: &str) -> Self {
        Self {
            root: TestDirectory::new(name),
        }
    }

    fn store(&self) -> (Store, LanguageEntry, String) {
        let store = Store::new(self.root.path.join("store"));
        let metadata = FactObject {
            version: 1,
            language: "python".into(),
            owner: String::new(),
            source_content_id: String::new(),
            adapter_version: "adapter-1".into(),
            schema_version: 1,
            analysis_config_id: "sha256:config".into(),
            records: Vec::new(),
        };

        let mut shared = metadata.clone();
        shared
            .records
            .push(FactRecord::Unresolved(UnresolvedRecord {
                attributes: None,
                candidate_name: None,
                candidate_namespace: None,
                expression: String::new(),
                owner: None,
                reason: "shared-record".into(),
                relation: String::new(),
                source: "node-a".into(),
                span: None,
            }));
        let shared_id = store.write_object(&shared).unwrap();

        let mut file = metadata;
        file.owner = "main.py".into();
        file.source_content_id = content_id(b"main.py");
        file.records = vec![
            FactRecord::Edge(EdgeRecord {
                attributes: None,
                owner: None,
                relation: "calls".into(),
                source: "node-z".into(),
                span: None,
                target: "node-a".into(),
            }),
            node("node-z", "function"),
            node("node-a", "file"),
            FactRecord::Unresolved(UnresolvedRecord {
                attributes: None,
                candidate_name: None,
                candidate_namespace: None,
                expression: String::new(),
                owner: None,
                reason: "file-record".into(),
                relation: String::new(),
                source: "node-z".into(),
                span: None,
            }),
        ];
        let file_id = store.write_object(&file).unwrap();
        let entry = LanguageEntry {
            language: "python".into(),
            adapter_version: "adapter-1".into(),
            adapter_fingerprint: String::new(),
            schema_version: 1,
            repository: "repo".into(),
            analysis_config_id: "sha256:config".into(),
            shared_object_id: shared_id,
            files: Some(vec![FileEntry {
                path: "main.py".into(),
                language: "python".into(),
                content_id: file.source_content_id,
                object_id: file_id,
            }]),
        };
        let snapshot = store
            .publish(&SnapshotManifest {
                version: 1,
                state_commit: "state".into(),
                languages: Some(vec![entry.clone()]),
            })
            .unwrap();
        (store, entry, snapshot)
    }
}

fn node(id: &str, kind: &str) -> FactRecord {
    FactRecord::Node(NodeRecord {
        attributes: None,
        content_id: None,
        id: id.into(),
        kind: kind.into(),
        name: id.into(),
        owner: None,
        path: "main.py".into(),
        qualified_name: id.into(),
        span: None,
    })
}

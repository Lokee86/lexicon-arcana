mod support;

use lexicon::interstack::{
    ADAPTER_VERSION, LANGUAGE, adapter_fingerprint, interstack_drifted, refresh_interstack,
};
use lexicon::{
    Analysis, FactHeader, FactRecord, LanguageEntry, NodeRecord, SnapshotManifest, SourceSpan,
    Store,
};

use support::TestDirectory;
use support::interstack::{test_id, write_fixture};

#[test]
fn fingerprint_and_drift_match_go_contract() {
    assert_eq!(LANGUAGE, "interstack");
    assert_eq!(ADAPTER_VERSION, "0.2.0");
    assert_eq!(
        adapter_fingerprint(),
        "sha256:6887e70d6ea4ec6055276ea5220a769ba9e4b01154ee3eec2c577874f6f26114"
    );

    let ordinary = entry("go", "test", "sha256:ordinary");
    let derived = entry(LANGUAGE, ADAPTER_VERSION, &adapter_fingerprint());

    assert!(interstack_drifted(&manifest(vec![ordinary.clone()])));
    assert!(!interstack_drifted(&manifest(vec![
        ordinary.clone(),
        derived.clone()
    ])));
    assert!(interstack_drifted(&manifest(vec![derived.clone()])));

    let mut stale = derived;
    stale.adapter_version = "0.1.0".into();
    assert!(interstack_drifted(&manifest(vec![ordinary, stale])));
}

#[test]
fn refresh_builds_shared_interstack_language_from_ordinary_nodes() {
    let root = TestDirectory::new("interstack-build");
    let source = root.path.join("source");
    let store = Store::new(root.path.join("store"));

    write_fixture(
        &source,
        "client/api.gd",
        "func auth_me_path():\n\treturn \"%s/api/auth/me\" % API_BASE\n",
    );
    write_fixture(
        &source,
        "services/api-server/config/routes.rb",
        "Rails.application.routes.draw do\n  namespace :api do\n    namespace :auth do\n      get \"me\", to: \"me#show\"\n    end\n  end\nend\n",
    );

    let gdscript = store
        .build_shared_language(
            &analysis(
                "gdscript",
                vec![
                    node(
                        'a',
                        "file",
                        "api.gd",
                        "client/api.gd",
                        "client/api.gd",
                        None,
                    ),
                    node(
                        'b',
                        "method",
                        "auth_me_path",
                        "client/api.gd",
                        "auth_me_path",
                        Some(1),
                    ),
                ],
            ),
            "sha256:analysis",
            "sha256:gdscript",
        )
        .unwrap();
    let ruby = store
        .build_shared_language(
            &analysis(
                "ruby",
                vec![
                    node(
                        'c',
                        "file",
                        "routes.rb",
                        "services/api-server/config/routes.rb",
                        "services/api-server/config/routes.rb",
                        None,
                    ),
                    node(
                        'd',
                        "method",
                        "show",
                        "services/api-server/app/controllers/api/auth/me_controller.rb",
                        "Api::Auth::MeController#show",
                        Some(4),
                    ),
                ],
            ),
            "sha256:analysis",
            "sha256:ruby",
        )
        .unwrap();

    let (manifest, summary) =
        refresh_interstack(&store, &source, manifest(vec![gdscript, ruby])).unwrap();

    assert_eq!(summary.http_contracts, 1);
    assert_eq!(summary.http_links, 1);
    let interstack = manifest.language(LANGUAGE).expect("interstack entry");
    assert_eq!(interstack.adapter_version, ADAPTER_VERSION);
    assert_eq!(interstack.adapter_fingerprint, adapter_fingerprint());
    assert_eq!(interstack.files.as_deref(), Some(&[][..]));
    assert!(!interstack.shared_object_id.is_empty());

    let object = store.load_object(&interstack.shared_object_id).unwrap();
    assert!(object.records.iter().any(|record| {
        matches!(record, FactRecord::Edge(edge) if edge.relation == "calls-endpoint")
    }));
    assert!(object.records.iter().any(|record| {
        matches!(record, FactRecord::Edge(edge) if edge.relation == "handled-by")
    }));
}

#[test]
fn refresh_removes_interstack_when_no_ordinary_language_remains() {
    let root = TestDirectory::new("interstack-remove");
    let store = Store::new(root.path.join("store"));
    let derived = entry(LANGUAGE, ADAPTER_VERSION, &adapter_fingerprint());
    let (manifest, summary) =
        refresh_interstack(&store, &root.path, manifest(vec![derived])).unwrap();
    assert!(manifest.language(LANGUAGE).is_none());
    assert_eq!(summary, Default::default());
}

fn analysis(language: &str, nodes: Vec<NodeRecord>) -> Analysis {
    Analysis {
        header: FactHeader {
            adapter_version: "test".into(),
            changed_files: None,
            language: language.into(),
            mode: Some("full".into()),
            record: "lexicon".into(),
            removed_files: None,
            repository: "space-rocks".into(),
            schema_version: 1,
            shared_complete: None,
        },
        records: nodes.into_iter().map(FactRecord::Node).collect(),
    }
}

fn node(
    character: char,
    kind: &str,
    name: &str,
    path: &str,
    qualified_name: &str,
    line: Option<u64>,
) -> NodeRecord {
    NodeRecord {
        attributes: None,
        content_id: None,
        id: test_id(character),
        kind: kind.into(),
        name: name.into(),
        owner: None,
        path: path.into(),
        qualified_name: qualified_name.into(),
        span: line.map(|line| SourceSpan {
            path: path.into(),
            start_line: line,
            start_column: 1,
            end_line: line,
            end_column: 2,
        }),
    }
}

fn manifest(languages: Vec<LanguageEntry>) -> SnapshotManifest {
    SnapshotManifest {
        version: lexicon::storage::SNAPSHOT_VERSION,
        state_commit: String::new(),
        languages: Some(languages),
    }
}

fn entry(language: &str, version: &str, fingerprint: &str) -> LanguageEntry {
    LanguageEntry {
        language: language.into(),
        adapter_version: version.into(),
        adapter_fingerprint: fingerprint.into(),
        schema_version: 1,
        repository: "repo".into(),
        analysis_config_id: "sha256:analysis".into(),
        shared_object_id: String::new(),
        files: Some(Vec::new()),
    }
}

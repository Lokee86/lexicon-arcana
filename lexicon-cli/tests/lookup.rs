use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use lexicon::{
    EdgeRecord, FactObject, FactRecord, FileEntry, LanguageEntry, NodeRecord, SnapshotManifest,
    Store, UnresolvedRecord, content_id, node_id, save_config, state_root,
};
use lexicon_cli::run;

#[test]
fn lookup_commands_use_published_snapshot_facts() {
    let fixture = Fixture::new();
    let repo = fixture.repository.to_str().unwrap();
    let caller = fixture.caller.as_str();
    let target = fixture.target.as_str();

    let (code, stdout, stderr) = invoke(["find", "pkg.target", "--repo", repo]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains(target));
    assert!(stdout.contains("pkg.target"));

    let (code, stdout, stderr) = invoke(["show", "pkg.target", "--repo", repo]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.contains(&format!("id: {target}")));
    assert!(stdout.contains("kind: function"));
    assert!(stdout.contains("qualified_name: pkg.target"));

    let (code, refs, stderr) = invoke(["refs", caller, "--repo", repo]);
    assert_eq!(code, 0, "{stderr}");
    assert!(refs.contains("out\tcalls\tpkg.target"));
    assert!(refs.contains("out\treferences\tpkg.choice"));
    assert!(refs.contains("out\tcalls\t? dynamic()\tdynamic-target"));

    let (code, calls, stderr) = invoke(["calls", caller, "--repo", repo]);
    assert_eq!(code, 0, "{stderr}");
    assert!(calls.contains("out\tcalls\tpkg.target"));
    assert!(calls.contains("out\tpossible-calls\tpkg.choice"));
    assert!(calls.contains("out\tcalls\t? dynamic()\tdynamic-target"));
    assert!(!calls.contains("\treferences\t"));

    let (code, incoming, stderr) = invoke([
        "calls",
        target,
        "--repo",
        repo,
        "--snapshot",
        &fixture.snapshot,
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert!(incoming.contains("in\tcalls\tpkg.caller"));
}

#[test]
fn lookup_commands_enforce_selector_and_result_bounds() {
    let fixture = Fixture::new();
    let repo = fixture.repository.to_str().unwrap();

    let (code, _, stderr) = invoke(["show", "target", "--repo", repo]);
    assert_eq!(code, 1);
    assert!(stderr.contains("ambiguous"));

    let (code, _, stderr) = invoke(["find", "target", "--repo", repo, "--limit", "201"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("--limit must not exceed 200"));

    let (code, stdout, stderr) = invoke(["find", "repo", "--repo", repo]);
    assert_eq!(code, 0, "{stderr}");
    assert!(stdout.is_empty());
}

fn invoke<const N: usize>(arguments: [&str; N]) -> (i32, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = run(
        arguments.into_iter().map(str::to_owned).collect(),
        &mut stdout,
        &mut stderr,
    );
    (
        code,
        String::from_utf8(stdout).unwrap(),
        String::from_utf8(stderr).unwrap(),
    )
}

struct Fixture {
    _root: TestDir,
    repository: PathBuf,
    snapshot: String,
    caller: String,
    target: String,
}

impl Fixture {
    fn new() -> Self {
        let root = TestDir::new();
        let repository = root.path().join("repository");
        let adapters = root.path().join("adapters");
        fs::create_dir_all(&repository).unwrap();
        fs::create_dir_all(&adapters).unwrap();
        save_config(&repository, &adapters).unwrap();
        let store = Store::new(state_root(&repository));

        let caller = id("caller");
        let target = id("target");
        let choice = id("choice");
        let duplicate = id("other-target");
        let a = write_object(
            &store,
            "a.py",
            vec![
                node(&caller, "caller", "pkg.caller", "a.py"),
                edge(&caller, &target, "calls", "a.py"),
                edge(&caller, &choice, "possible-calls", "a.py"),
                edge(&caller, &choice, "references", "a.py"),
                unresolved(&caller, "a.py"),
            ],
        );
        let b = write_object(
            &store,
            "b.py",
            vec![
                node(&target, "target", "pkg.target", "b.py"),
                node(&choice, "choice", "pkg.choice", "b.py"),
                node(&duplicate, "target", "other.target", "b.py"),
            ],
        );
        let snapshot = store
            .publish(&SnapshotManifest {
                version: 1,
                state_commit: "state".into(),
                languages: Some(vec![language(vec![a, b])]),
            })
            .unwrap();
        Self {
            _root: root,
            repository,
            snapshot,
            caller,
            target,
        }
    }
}

fn id(name: &str) -> String {
    node_id("python", "function", name)
}

fn node(id: &str, name: &str, qualified: &str, path: &str) -> FactRecord {
    FactRecord::Node(NodeRecord {
        attributes: None,
        content_id: None,
        id: id.into(),
        kind: "function".into(),
        name: name.into(),
        owner: Some(path.into()),
        path: path.into(),
        qualified_name: qualified.into(),
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

fn unresolved(source: &str, path: &str) -> FactRecord {
    FactRecord::Unresolved(UnresolvedRecord {
        attributes: None,
        candidate_name: Some("dynamic".into()),
        candidate_namespace: None,
        expression: "dynamic()".into(),
        owner: Some(path.into()),
        reason: "dynamic-target".into(),
        relation: "calls".into(),
        source: source.into(),
        span: None,
    })
}

fn write_object(store: &Store, path: &str, records: Vec<FactRecord>) -> FileEntry {
    let content = content_id(path.as_bytes());
    let object = FactObject {
        version: 1,
        language: "python".into(),
        owner: path.into(),
        source_content_id: content.clone(),
        adapter_version: "test".into(),
        schema_version: 1,
        analysis_config_id: "config".into(),
        records,
    };
    FileEntry {
        path: path.into(),
        language: "python".into(),
        content_id: content,
        object_id: store.write_object(&object).unwrap(),
    }
}

fn language(files: Vec<FileEntry>) -> LanguageEntry {
    LanguageEntry {
        language: "python".into(),
        adapter_version: "test".into(),
        adapter_fingerprint: String::new(),
        schema_version: 1,
        repository: "repo".into(),
        analysis_config_id: "config".into(),
        shared_object_id: String::new(),
        dependency_index_id: String::new(),
        files: Some(files),
    }
}

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lexicon-cli-lookup-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

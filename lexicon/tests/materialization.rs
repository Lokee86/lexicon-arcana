mod support;

use lexicon::{Analysis, SnapshotManifest, SourceFile, Store, content_id};

use support::TestDirectory;

const REPO: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const FILE_A: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
const FN_A: &str = "sha256:3333333333333333333333333333333333333333333333333333333333333333";
const SCOPED_REPO: &str = "sha256:4444444444444444444444444444444444444444444444444444444444444444";
const FILE_A_NEW: &str = "sha256:5555555555555555555555555555555555555555555555555555555555555555";

#[test]
fn full_materialization_partitions_owned_and_shared_facts() {
    let directory = TestDirectory::new("full-materialization");
    let store = Store::new(&directory.path);
    let analysis = Analysis::parse(&full_stream()).expect("Go-shaped full stream");
    let sources = vec![
        source("b.py", b"other = 1\n"),
        source("a.py", b"def run(): pass\n"),
    ];

    let entry = store
        .build_full_language(
            &analysis,
            &sources,
            "python",
            "sha256:config",
            "sha256:adapter",
        )
        .unwrap();
    let files = entry.files.as_ref().unwrap();
    assert_eq!(
        files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        vec!["a.py", "b.py"]
    );

    let a = &files[0];
    assert_eq!(a.content_id, content_id(b"def run(): pass\n"));
    let a_object = store.load_object(&a.object_id).unwrap();
    assert_eq!(a_object.owner, "a.py");
    assert_eq!(a_object.records.len(), 4);

    let b_object = store.load_object(&files[1].object_id).unwrap();
    assert_eq!(b_object.owner, "b.py");
    assert!(b_object.records.is_empty());

    let shared = store.load_object(&entry.shared_object_id).unwrap();
    assert_eq!(shared.owner, "");
    assert_eq!(shared.records.len(), 1);
    assert!(matches!(&shared.records[0], lexicon::FactRecord::Node(node) if node.id == REPO));
}

#[test]
fn incremental_materialization_reuses_unchanged_objects_and_shared_facts() {
    let directory = TestDirectory::new("incremental-materialization");
    let store = Store::new(&directory.path);
    let full = Analysis::parse(&full_stream()).unwrap();
    let previous = store
        .build_full_language(
            &full,
            &[
                source("a.py", b"value = 1\n"),
                source("b.py", b"other = 1\n"),
            ],
            "python",
            "sha256:config",
            "sha256:adapter",
        )
        .unwrap();

    let old_a = file(&previous, "a.py").clone();
    let old_b = file(&previous, "b.py").clone();
    let old_shared = previous.shared_object_id.clone();
    let incremental = Analysis::parse(&incremental_stream()).unwrap();

    let updated = store
        .build_incremental_language(
            &previous,
            &incremental,
            &[source("a.py", b"value = 2\n")],
            "sha256:config",
            "sha256:adapter",
            &["a.py".into()],
            &[],
            false,
        )
        .unwrap();

    let new_a = file(&updated, "a.py");
    assert_ne!(new_a.object_id, old_a.object_id);
    assert_ne!(new_a.content_id, old_a.content_id);
    assert_eq!(file(&updated, "b.py"), &old_b);
    assert_eq!(updated.shared_object_id, old_shared);
}

#[test]
fn manifest_language_mutation_matches_go_sorting_behavior() {
    let directory = TestDirectory::new("manifest-language");
    let store = Store::new(&directory.path);
    let python = store
        .build_shared_language(
            &Analysis::parse(&full_stream()).unwrap(),
            "config",
            "adapter",
        )
        .unwrap();
    let mut rust = python.clone();
    rust.language = "rust".into();

    let manifest = SnapshotManifest {
        version: 1,
        state_commit: "state".into(),
        languages: None,
    }
    .with_language(rust)
    .with_language(python.clone());
    assert_eq!(
        manifest
            .languages
            .as_ref()
            .unwrap()
            .iter()
            .map(|entry| entry.language.as_str())
            .collect::<Vec<_>>(),
        vec!["python", "rust"]
    );
    assert_eq!(manifest.language("python"), Some(&python));

    let removed = manifest.without_language("python");
    assert!(removed.language("python").is_none());
    assert_eq!(removed.languages.unwrap().len(), 1);
}

fn file<'a>(entry: &'a lexicon::LanguageEntry, path: &str) -> &'a lexicon::FileEntry {
    entry
        .files
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|file| file.path == path)
        .unwrap()
}

fn source(path: &str, content: &[u8]) -> SourceFile {
    SourceFile {
        path: path.into(),
        content: content.to_vec(),
    }
}

fn full_stream() -> String {
    concat!(
        "{\"adapter_version\":\"test\",\"language\":\"python\",\"mode\":\"full\",\"record\":\"lexicon\",\"repository\":\"repo\",\"schema_version\":1}\n",
        "{\"id\":\"$REPO\",\"kind\":\"repository\",\"name\":\"repo\",\"path\":\".\",\"qualified_name\":\"repo\",\"record\":\"node\"}\n",
        "{\"id\":\"$FILE_A\",\"kind\":\"file\",\"name\":\"a.py\",\"owner\":\"a.py\",\"path\":\"a.py\",\"qualified_name\":\"a.py\",\"record\":\"node\"}\n",
        "{\"id\":\"$FN_A\",\"kind\":\"function\",\"name\":\"run\",\"owner\":\"a.py\",\"path\":\"a.py\",\"qualified_name\":\"a.run\",\"record\":\"node\"}\n",
        "{\"record\":\"edge\",\"relation\":\"defines\",\"source\":\"$FILE_A\",\"target\":\"$FN_A\"}\n",
        "{\"candidate_name\":\"missing\",\"expression\":\"missing()\",\"reason\":\"missing-target\",\"record\":\"unresolved\",\"relation\":\"calls\",\"source\":\"$FN_A\"}\n"
    )
    .replace("$REPO", REPO)
    .replace("$FILE_A", FILE_A)
    .replace("$FN_A", FN_A)
}

fn incremental_stream() -> String {
    concat!(
        "{\"adapter_version\":\"test\",\"changed_files\":[\"a.py\"],\"language\":\"python\",\"mode\":\"incremental\",\"record\":\"lexicon\",\"removed_files\":[],\"repository\":\"repo\",\"schema_version\":1,\"shared_complete\":true}\n",
        "{\"id\":\"$SCOPED_REPO\",\"kind\":\"repository\",\"name\":\"repo\",\"path\":\".\",\"qualified_name\":\"repo\",\"record\":\"node\"}\n",
        "{\"id\":\"$FILE_A_NEW\",\"kind\":\"file\",\"name\":\"a.py\",\"owner\":\"a.py\",\"path\":\"a.py\",\"qualified_name\":\"a.py\",\"record\":\"node\"}\n"
    )
    .replace("$SCOPED_REPO", SCOPED_REPO)
    .replace("$FILE_A_NEW", FILE_A_NEW)
}

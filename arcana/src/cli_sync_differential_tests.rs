use std::fs;

use arcana::repository::RepositorySnapshot;
use serde_json::json;

use super::cli::SyncCommand;
use super::cli_sync::run_sync;
use super::cli_sync_compact_test_support::{
    TestDirectory, edge, node, sha_id, write_object, write_snapshot,
};

#[test]
fn managed_incremental_store_matches_clean_rebuild() {
    let temp = TestDirectory::new();
    let lexicon = temp.path.join(".lexicon");
    let incremental_state = temp.path.join(".arcana-incremental");
    let clean_state = temp.path.join(".arcana-clean");
    fs::create_dir_all(lexicon.join("objects")).unwrap();
    fs::create_dir_all(lexicon.join("snapshots")).unwrap();

    let repository = sha_id("diff-repository");
    let source = sha_id("diff-source");
    let first_target = sha_id("diff-first-target");
    let second_target = sha_id("diff-second-target");
    let third_target = sha_id("diff-third-target");
    let shared = write_object(
        &lexicon,
        None,
        None,
        vec![
            node(&repository, "repository", ".lexicon-repository", "repo"),
            node(&first_target, "function", "shared", "first"),
            node(&second_target, "function", "shared", "second"),
            node(&third_target, "function", "shared", "third"),
        ],
    );

    let first_content = sha_id("diff-content-one");
    let first_file = write_object(
        &lexicon,
        Some("src/a.go"),
        Some(&first_content),
        vec![
            node(&source, "function", "src/a.go", "source-old"),
            edge(&source, &first_target),
            edge(&source, &second_target),
            json!({
                "record": "unresolved",
                "source": source,
                "relation": "calls",
                "expression": "old-one()",
                "reason": "dynamic-target"
            }),
            json!({
                "record": "unresolved",
                "source": source,
                "relation": "calls",
                "expression": "old-two()",
                "reason": "dynamic-target"
            }),
        ],
    );
    let first_id = write_snapshot(&lexicon, "diff-first", &shared, &first_content, &first_file);
    fs::write(lexicon.join("CURRENT"), format!("{first_id}\n")).unwrap();
    run_sync(&SyncCommand {
        lexicon: lexicon.clone(),
        state: incremental_state.clone(),
        register: false,
    })
    .unwrap();

    let second_content = sha_id("diff-content-two");
    let second_file = write_object(
        &lexicon,
        Some("src/a.go"),
        Some(&second_content),
        vec![
            node(&source, "function", "src/a.go", "source-new"),
            edge(&source, &second_target),
            edge(&source, &second_target),
            edge(&source, &third_target),
            json!({
                "record": "unresolved",
                "source": source,
                "relation": "calls",
                "expression": "new-target()",
                "reason": "dynamic-target"
            }),
        ],
    );
    let second_id = write_snapshot(
        &lexicon,
        "diff-second",
        &shared,
        &second_content,
        &second_file,
    );
    fs::write(lexicon.join("CURRENT"), format!("{second_id}\n")).unwrap();

    let incremental_summary = run_sync(&SyncCommand {
        lexicon: lexicon.clone(),
        state: incremental_state.clone(),
        register: false,
    })
    .unwrap();
    assert!(
        incremental_summary.contains("mode=overlay"),
        "{incremental_summary}"
    );

    let clean_summary = run_sync(&SyncCommand {
        lexicon,
        state: clean_state.clone(),
        register: false,
    })
    .unwrap();
    assert!(clean_summary.contains("mode=rebuild"), "{clean_summary}");

    let digest = second_id.strip_prefix("sha256:").unwrap();
    let incremental_output = incremental_state.join("snapshots").join(digest);
    let clean_output = clean_state.join("snapshots").join(digest);
    assert_eq!(
        fs::read(incremental_output.join("repository.arcana")).unwrap(),
        fs::read(clean_output.join("repository.arcana")).unwrap()
    );

    let incremental =
        RepositorySnapshot::open(incremental_output.join("repository.manifest")).unwrap();
    let clean = RepositorySnapshot::open(clean_output.join("repository.manifest")).unwrap();
    assert_eq!(incremental.facts(), clean.facts());
    assert_eq!(
        incremental.manifest().repository_id,
        clean.manifest().repository_id
    );
    assert_eq!(
        incremental.manifest().repository_store_checksum,
        clean.manifest().repository_store_checksum
    );
    assert_eq!(
        incremental.manifest().node_count,
        clean.manifest().node_count
    );
    assert_eq!(
        incremental.manifest().edge_count,
        clean.manifest().edge_count
    );
    assert_eq!(
        incremental.manifest().unresolved_count,
        clean.manifest().unresolved_count
    );
    assert_eq!(incremental.facts().edges.len(), 2);
    assert_eq!(incremental.facts().unresolved.len(), 1);
}

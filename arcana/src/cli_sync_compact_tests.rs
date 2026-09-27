use std::fs;

use arcana::repository::RepositorySnapshot;

use super::cli::SyncCommand;
use super::cli_sync::run_sync;
use super::cli_sync_compact_test_support::{
    TestDirectory, edge, node, node_key, sha_id, write_object, write_snapshot,
};

#[test]
fn managed_sync_uses_compact_overlay_path_for_stable_file_nodes() {
    let temp = TestDirectory::new();
    let lexicon = temp.path.join(".lexicon");
    let state = temp.path.join(".arcana");
    fs::create_dir_all(lexicon.join("objects")).unwrap();
    fs::create_dir_all(lexicon.join("snapshots")).unwrap();

    let repository = sha_id("repository");
    let source = sha_id("source");
    let first_target = sha_id("first-target");
    let second_target = sha_id("second-target");
    let shared = write_object(
        &lexicon,
        None,
        None,
        vec![
            node(&repository, "repository", ".lexicon-repository", "repo"),
            node(&first_target, "function", "shared", "first"),
            node(&second_target, "function", "shared", "second"),
        ],
    );

    let first_content = sha_id("content-one");
    let first_file = write_object(
        &lexicon,
        Some("src/a.go"),
        Some(&first_content),
        vec![
            node(&source, "function", "src/a.go", "source"),
            edge(&source, &first_target),
        ],
    );
    let first_id = write_snapshot(&lexicon, "first", &shared, &first_content, &first_file);
    fs::write(
        lexicon.join("CURRENT"),
        format!(
            "{first_id}
"
        ),
    )
    .unwrap();
    run_sync(&SyncCommand {
        lexicon: lexicon.clone(),
        state: state.clone(),
        register: false,
    })
    .unwrap();

    let second_content = sha_id("content-two");
    let second_file = write_object(
        &lexicon,
        Some("src/a.go"),
        Some(&second_content),
        vec![
            node(&source, "function", "src/a.go", "source"),
            edge(&source, &second_target),
        ],
    );
    let second_id = write_snapshot(&lexicon, "second", &shared, &second_content, &second_file);
    fs::write(
        lexicon.join("CURRENT"),
        format!(
            "{second_id}
"
        ),
    )
    .unwrap();

    let summary = run_sync(&SyncCommand {
        lexicon,
        state: state.clone(),
        register: false,
    })
    .unwrap();
    assert!(summary.contains("mode=overlay"), "{summary}");

    let output = state
        .join("snapshots")
        .join(second_id.strip_prefix("sha256:").unwrap());
    assert!(output.join("overlay.arcana").is_file());
    let snapshot = RepositorySnapshot::open(output.join("repository.manifest")).unwrap();
    assert_eq!(snapshot.facts().edges.len(), 1);
    assert_eq!(
        snapshot.facts().edges[0].target.as_u64(),
        node_key(&second_target)
    );
}

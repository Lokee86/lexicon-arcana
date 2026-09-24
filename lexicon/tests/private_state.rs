mod support;

use std::fs;
use std::process::Command;

use lexicon::StateRepository;

use support::TestDirectory;

#[test]
fn private_state_keeps_one_replaceable_commit_and_reports_source_changes() {
    let root = TestDirectory::new("git-state");
    let repository = StateRepository::ensure(&root.path).unwrap();
    let source = root.path.join("source");
    fs::create_dir_all(&source).unwrap();
    let path = source.join("main.py");
    fs::write(&path, b"value = 1\n").unwrap();

    repository.stage_all().unwrap();
    repository.commit_state().unwrap();
    let first_head = repository.head().unwrap();

    fs::write(&path, b"value = 2\n").unwrap();
    repository.stage_source().unwrap();
    let changes = repository.source_changes().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].status, "M");
    assert_eq!(changes[0].old, "");
    assert_eq!(changes[0].new, "main.py");

    repository.stage_all().unwrap();
    repository.commit_state().unwrap();
    let second_head = repository.head().unwrap();
    assert_ne!(first_head, second_head);
    assert_eq!(reachable_commit_count(&root.path), "1");
}

#[test]
fn private_state_reports_renames_with_normalized_old_and_new_paths() {
    let root = TestDirectory::new("git-rename");
    let repository = StateRepository::ensure(&root.path).unwrap();
    let source = root.path.join("source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("old.py"), b"value = 1\n").unwrap();
    repository.stage_all().unwrap();
    repository.commit_state().unwrap();

    fs::rename(source.join("old.py"), source.join("new.py")).unwrap();
    repository.stage_source().unwrap();
    let changes = repository.source_changes().unwrap();
    assert_eq!(changes.len(), 1);
    assert!(changes[0].status.starts_with('R'));
    assert_eq!(changes[0].old, "old.py");
    assert_eq!(changes[0].new, "new.py");
}

#[test]
fn private_state_without_head_has_no_source_changes() {
    let root = TestDirectory::new("git-no-head");
    let repository = StateRepository::ensure(&root.path).unwrap();
    assert!(!repository.has_head());
    assert_eq!(repository.head_option().unwrap(), None);
    assert!(repository.source_changes().unwrap().is_empty());
}

#[test]
fn private_state_can_be_reopened() {
    let root = TestDirectory::new("git-open");
    let repository = StateRepository::ensure(&root.path).unwrap();
    repository.stage_all().unwrap();
    repository.commit_state().unwrap();

    let reopened = StateRepository::open(&root.path).unwrap();
    assert_eq!(reopened.head().unwrap(), repository.head().unwrap());
}

fn reachable_commit_count(root: &std::path::Path) -> String {
    let output = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

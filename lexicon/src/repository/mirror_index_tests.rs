use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::repository::walk::relevant_files;
use crate::repository::{IgnorePolicy, SourceMirror, StateRepository};

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    state: StateRepository,
    mirror: SourceMirror,
}

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("lexicon-index-phase1-{nonce}"));
        let source = root.join("repo");
        fs::create_dir_all(&source).unwrap();
        let state = StateRepository::ensure(root.join("state")).unwrap();
        let mirror = SourceMirror::new(state.root().join("source"));
        let fixture = Self {
            root,
            source,
            state,
            mirror,
        };
        fixture.write("a.py", "a = 1\n");
        fixture.write("b.py", "b = 1\n");
        fixture.git(&["init", "-q"]);
        fixture.git(&["config", "core.autocrlf", "false"]);
        fixture.git(&["config", "user.email", "test@example.invalid"]);
        fixture.git(&["config", "user.name", "Fixture"]);
        fixture.git(&["add", "-A"]);
        fixture.git(&["commit", "-qm", "initial"]);
        fixture.publish_mirror();
        fixture
    }

    fn write(&self, path: &str, content: &str) {
        let destination = self.source.join(path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::write(destination, content).unwrap();
    }

    fn git(&self, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.source)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?}: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn publish_mirror(&self) {
        self.mirror.sync_all(&self.source).unwrap();
        self.state.stage_all().unwrap();
        self.state.commit_state().unwrap();
    }

    fn desired(&self) -> BTreeMap<PathBuf, PathBuf> {
        relevant_files(
            &self.source,
            &self.source,
            &IgnorePolicy::load(&self.source).unwrap(),
        )
        .unwrap()
    }

    fn skipped(&self) -> BTreeSet<PathBuf> {
        unchanged_files(self.mirror.root(), &self.source, &self.desired()).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn paths(values: &[&str]) -> BTreeSet<PathBuf> {
    values.iter().map(PathBuf::from).collect()
}

#[test]
fn clean_git_trees_skip_without_source_content_hashes() {
    let fixture = Fixture::new();
    assert_eq!(fixture.skipped(), paths(&["a.py", "b.py"]));
    fixture.write("a.py", "a = 2\n");
    assert_eq!(fixture.skipped(), paths(&["b.py"]));
    fixture.mirror.sync_all(&fixture.source).unwrap();
    // An interrupted scan leaves private mirror dirty but clean unrelated paths
    // must still qualify against the previous immutable state commit.
    assert_eq!(fixture.skipped(), paths(&["b.py"]));
    fixture.state.stage_all().unwrap();
    fixture.state.commit_state().unwrap();
    assert_eq!(fixture.skipped(), paths(&["b.py"]));
    fixture.write("a.py", "a = 3\n");
    assert_eq!(fixture.skipped(), paths(&["b.py"]));
    fixture.publish_mirror();
    assert_eq!(fixture.skipped(), paths(&["b.py"]));
    // Reverting to the original source HEAD differs from the private state HEAD.
    fixture.write("a.py", "a = 1\n");
    assert_eq!(fixture.skipped(), paths(&["b.py"]));
    fixture.publish_mirror();
    assert_eq!(fixture.skipped(), paths(&["a.py", "b.py"]));
}

#[test]
fn private_corruption_disqualifies_only_the_corrupt_file() {
    let fixture = Fixture::new();
    fs::write(fixture.mirror.root().join("a.py"), b"corrupt\n").unwrap();
    assert_eq!(fixture.skipped(), paths(&["b.py"]));
    fixture.mirror.sync_all(&fixture.source).unwrap();
    assert_eq!(
        fs::read(fixture.mirror.root().join("a.py")).unwrap(),
        b"a = 1\n"
    );
    assert_eq!(fixture.skipped(), paths(&["a.py", "b.py"]));
}

#[test]
fn staged_unstaged_and_special_index_flags_cannot_hide_edits() {
    let fixture = Fixture::new();
    fixture.write("a.py", "a = 2\n");
    fixture.git(&["add", "a.py"]);
    assert_eq!(fixture.skipped(), paths(&["b.py"]));
    fixture.write("b.py", "b = 2\n");
    assert!(fixture.skipped().is_empty());
    fixture.git(&["reset", "-q", "--hard", "HEAD"]);
    fixture.git(&["update-index", "--assume-unchanged", "a.py"]);
    assert_eq!(fixture.skipped(), paths(&["b.py"]));
    fixture.git(&["update-index", "--no-assume-unchanged", "a.py"]);
    fixture.git(&["update-index", "--skip-worktree", "b.py"]);
    assert_eq!(fixture.skipped(), paths(&["a.py"]));
    fixture.git(&["update-index", "--no-skip-worktree", "b.py"]);
}

#[test]
fn committed_source_changes_ignore_policy_add_delete_and_rename_are_reconciled() {
    let fixture = Fixture::new();
    fixture.write("a.py", "a = 9\n");
    fixture.git(&["add", "-A"]);
    fixture.git(&["commit", "-qm", "edit"]);
    assert_eq!(fixture.skipped(), paths(&["b.py"]));
    fixture.publish_mirror();
    assert_eq!(fixture.skipped(), paths(&["a.py", "b.py"]));
    fixture.write("extra.py", "extra = 1\n");
    assert_eq!(fixture.skipped(), paths(&["a.py", "b.py"]));
    fixture.mirror.sync_all(&fixture.source).unwrap();
    assert_eq!(
        fs::read(fixture.mirror.root().join("extra.py")).unwrap(),
        b"extra = 1\n"
    );
    fixture.write(".lexiconignore", "b.py\n");
    fixture.mirror.sync_all(&fixture.source).unwrap();
    assert!(!fixture.mirror.root().join("b.py").exists());
    fs::remove_file(fixture.source.join("extra.py")).unwrap();
    fs::rename(
        fixture.source.join("a.py"),
        fixture.source.join("renamed.py"),
    )
    .unwrap();
    fixture.mirror.sync_all(&fixture.source).unwrap();
    assert!(!fixture.mirror.root().join("a.py").exists());
    assert!(!fixture.mirror.root().join("extra.py").exists());
    assert_eq!(
        fs::read(fixture.mirror.root().join("renamed.py")).unwrap(),
        b"a = 9\n"
    );
}

#[test]
fn non_git_and_absent_private_commit_fall_back_to_verified_copy() {
    let fixture = Fixture::new();
    let foreign = fixture.root.join("foreign");
    fs::create_dir_all(&foreign).unwrap();
    fs::write(foreign.join("a.py"), "value = 1\n").unwrap();
    let desired =
        relevant_files(&foreign, &foreign, &IgnorePolicy::load(&foreign).unwrap()).unwrap();
    assert!(unchanged_files(fixture.mirror.root(), &foreign, &desired).is_none());
    // Without a published private HEAD, source Git blobs alone are insufficient.
    let fresh = SourceMirror::new(fixture.root.join("uncommitted").join("source"));
    assert!(unchanged_files(fresh.root(), &fixture.source, &fixture.desired()).is_none());
    let mirror = SourceMirror::new(fixture.root.join("plain-mirror"));
    mirror.sync_all(&foreign).unwrap();
    fs::write(foreign.join("a.py"), "value = 2\n").unwrap();
    mirror.sync_all(&foreign).unwrap();
    assert_eq!(
        fs::read(mirror.root().join("a.py")).unwrap(),
        b"value = 2\n"
    );
}

#[test]
fn converted_line_endings_are_not_mistaken_for_the_same_blob() {
    let fixture = Fixture::new();
    fixture.write(".gitattributes", "*.py text eol=crlf\n");
    fixture.git(&["add", ".gitattributes"]);
    fixture.git(&["commit", "-qm", "declare checkout conversion"]);
    fs::remove_file(fixture.source.join("a.py")).unwrap();
    fixture.git(&["checkout", "HEAD", "--", "a.py"]);
    assert_eq!(fs::read(fixture.source.join("a.py")).unwrap(), b"a = 1\r\n");
    // The source and mirror HEAD blobs are identical, and Git reports a
    // clean working tree, but the checked-out source bytes are not identical.
    assert_eq!(fixture.skipped(), paths(&["b.py"]));
    fixture.mirror.sync_all(&fixture.source).unwrap();
    assert_eq!(
        fs::read(fixture.mirror.root().join("a.py")).unwrap(),
        b"a = 1\r\n"
    );
}

#[test]
fn git_smudge_and_encoding_attributes_force_verified_comparison() {
    let fixture = Fixture::new();
    fixture.write(".gitattributes", "b.py filter=example\n");
    fixture.git(&["add", ".gitattributes"]);
    fixture.git(&["commit", "-qm", "add custom filter attribute"]);
    assert_eq!(fixture.skipped(), paths(&["a.py"]));
}

#[test]
fn nested_git_source_prefix_and_nonascii_paths_work() {
    let fixture = Fixture::new();
    let child = fixture.source.join("nested");
    fs::create_dir_all(&child).unwrap();
    fs::write(child.join("café.py"), "value = 1\n").unwrap();
    fixture.git(&["add", "-A"]);
    fixture.git(&["commit", "-qm", "nested"]);
    // Git tree proof uses the nested working directory's --show-prefix.
    let index_state = StateRepository::ensure(fixture.root.join("nested-state")).unwrap();
    let nested = SourceMirror::new(index_state.root().join("source"));
    nested.sync_all(&child).unwrap();
    index_state.stage_all().unwrap();
    index_state.commit_state().unwrap();
    let desired = relevant_files(&child, &child, &IgnorePolicy::load(&child).unwrap()).unwrap();
    assert_eq!(
        unchanged_files(nested.root(), &child, &desired).unwrap(),
        paths(&["café.py"])
    );
    fs::write(child.join("café.py"), "value = 2\n").unwrap();
    assert!(
        unchanged_files(nested.root(), &child, &desired)
            .unwrap()
            .is_empty()
    );
}

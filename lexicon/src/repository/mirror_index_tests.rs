use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use crate::repository::{SourceMirror, StateRepository};

#[test]
fn clean_index_matches_are_trusted_but_dirty_mirrors_fall_back() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("lexicon-mirror-index-{nonce}"));
    let source = root.join("repo");
    let state_root = root.join("state");
    let mirror_root = state_root.join("source");
    fs::create_dir_all(&source).expect("create source");
    fs::write(source.join("a.py"), b"print('a')\n").expect("write source");

    let state = StateRepository::ensure(&state_root).expect("state repository");
    let mirror = SourceMirror::new(&mirror_root);
    mirror.sync_all(&source).expect("initial mirror");
    state.stage_all().expect("stage state");
    state.commit_state().expect("commit state");

    let desired = crate::repository::walk::relevant_files(
        &source,
        &source,
        &crate::repository::IgnorePolicy::load(&source).expect("ignore policy"),
    )
    .expect("desired files");
    let unchanged = unchanged_files(&mirror_root, &source, &desired).expect("clean fast path");
    assert!(unchanged.contains(Path::new("a.py")));

    fs::write(mirror_root.join("a.py"), b"corrupt\n").expect("corrupt mirror");
    assert!(unchanged_files(&mirror_root, &source, &desired).is_none());
    mirror.sync_all(&source).expect("repair mirror");
    assert_eq!(
        fs::read(mirror_root.join("a.py")).expect("read repaired mirror"),
        b"print('a')\n"
    );

    let _ = fs::remove_dir_all(root);
}

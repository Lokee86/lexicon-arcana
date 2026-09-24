mod support;

use std::fs;

use lexicon::{IgnorePolicy, ignored_directory, prepare_state_directory};

use support::TestDirectory;

#[test]
fn permanent_ignored_directories_match_go() {
    for name in [
        ".ddocs",
        ".lexicon",
        ".arcana",
        ".grimoire",
        ".pitlord",
        ".cantrip",
        ".homunculus",
        ".incubus",
        ".ritual",
        ".warlock",
        ".git",
        ".astro",
        "node_modules",
        "target",
    ] {
        assert!(ignored_directory(name), "{name} should be ignored");
    }
}

#[test]
fn ignore_policy_matches_go_patterns_and_permanent_exclusions() {
    let root = TestDirectory::new("ignore-policy");
    fs::write(
        root.path.join(".lexiconignore"),
        b"*.py\n!keep.py\nnested/*.go\n!nested/keep.go\ncache/\n/root-only/\nlocked/\n!locked/keep.py\n!.git/keep.go\n",
    )
    .unwrap();
    let policy = IgnorePolicy::load(&root.path).unwrap();

    for (relative, is_dir, expected) in [
        ("main.py", false, true),
        ("keep.py", false, false),
        ("nested/drop.go", false, true),
        ("nested/keep.go", false, false),
        ("cache/data.py", false, true),
        ("root-only/data.go", false, true),
        ("nested/root-only/data.go", false, false),
        ("locked/keep.py", false, true),
        (".git/keep.go", false, true),
        ("src/visible.go", false, false),
        ("cache", true, true),
        (".lexiconignore", false, false),
    ] {
        assert_eq!(
            policy.ignored(&root.path.join(relative), is_dir),
            expected,
            "{relative}"
        );
    }
}

#[test]
fn prepare_state_directory_is_idempotent_and_preserves_crlf() {
    let root = TestDirectory::new("state-ignore");
    fs::write(root.path.join(".gitignore"), b"node_modules/\r\n").unwrap();
    let state = root.path.join(".warlock").join("tools").join("lexicon");

    prepare_state_directory(&root.path, &state).unwrap();
    prepare_state_directory(&root.path, &state).unwrap();

    assert!(state.is_dir());
    let data = fs::read(root.path.join(".gitignore")).unwrap();
    let text = String::from_utf8(data).unwrap();
    assert_eq!(text.matches("/.warlock/").count(), 1);
    assert!(text.contains("\r\n/.warlock/\r\n"));
}

#[test]
fn prepare_state_directory_keeps_normal_parent_scoped_and_external_state_external() {
    let root = TestDirectory::new("state-scope");
    let nested = root.path.join("docs").join(".ddocs");
    prepare_state_directory(&root.path, &nested).unwrap();
    assert_eq!(
        fs::read_to_string(root.path.join(".gitignore")).unwrap(),
        "/docs/.ddocs/\n"
    );

    let other_root = TestDirectory::new("external-state");
    let repository = TestDirectory::new("external-repo");
    let external = other_root.path.join(".lexicon");
    prepare_state_directory(&repository.path, &external).unwrap();
    assert!(!repository.path.join(".gitignore").exists());
}

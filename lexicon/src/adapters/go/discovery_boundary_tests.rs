use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{AdapterRequest, content_id};

use super::{discovery, helper_arguments, helper_environment, semantic_request};

#[test]
fn exclusions_and_helper_inventory_are_rust_owned() {
    let root = TempDirectory::new("inventory");
    write(&root.path, "go.mod", "module example.com/inventory\n");
    write(&root.path, "main.go", "package main\n");
    write(&root.path, "visible/keep.go", "package visible\n");
    for excluded in [
        ".git",
        ".worktrees",
        ".workingtrees",
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
        "vendor",
    ] {
        write(
            &root.path,
            &format!("{excluded}/ignored.go"),
            "package ignored\n",
        );
    }

    let inventory = discovery::discover(&root.path).unwrap();
    assert_eq!(
        inventory.semantic_files(),
        vec!["go.mod", "main.go", "visible/keep.go"]
    );
    assert_eq!(inventory.directories, vec!["visible"]);

    let manifest = inventory
        .files
        .iter()
        .find(|file| file.path == "go.mod")
        .unwrap();
    assert_eq!(
        manifest.manifest_content.as_deref(),
        Some(b"module example.com/inventory\n".as_slice())
    );
    assert_eq!(
        manifest.content_id,
        content_id(b"module example.com/inventory\n")
    );

    let main = inventory
        .files
        .iter()
        .find(|file| file.path == "main.go")
        .unwrap();
    assert!(main.manifest_content.is_none());
    assert_eq!(main.content_id, content_id(b"package main\n"));

    let visible = inventory
        .files
        .iter()
        .find(|file| file.path == "visible/keep.go")
        .unwrap();
    assert!(visible.manifest_content.is_none());
    assert_eq!(visible.content_id, content_id(b"package visible\n"));

    let request = semantic_request(
        &fs::canonicalize(&root.path).unwrap(),
        &inventory,
        &AdapterRequest {
            language: "go".into(),
            repository: root.path.clone(),
            workers: 4,
            shards: 8,
            merge_fan_in: 4,
            ..AdapterRequest::default()
        },
    )
    .unwrap();
    assert_eq!(request.files, vec!["go.mod", "main.go", "visible/keep.go"]);
    assert_eq!(request.modules[0].root, ".");
    assert_eq!(helper_arguments()[1], "2");
    assert_eq!(helper_environment().len(), 2);
}

#[cfg(unix)]
#[test]
fn symlinked_directories_are_not_followed() {
    use std::os::unix::fs::symlink;

    let root = TempDirectory::new("symlink");
    write(&root.path, "go.mod", "module example.com/symlink\n");
    let outside = TempDirectory::new("outside");
    write(&outside.path, "hidden.go", "package hidden\n");
    symlink(&outside.path, root.path.join("linked")).unwrap();

    let inventory = discovery::discover(&root.path).unwrap();
    assert_eq!(inventory.semantic_files(), vec!["go.mod"]);
    assert!(inventory.directories.is_empty());
}

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

struct TempDirectory {
    path: PathBuf,
}

impl TempDirectory {
    fn new(name: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lexicon-go-phase3-{name}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

mod support;

use std::fs;
use std::path::PathBuf;

use lexicon::SourceMirror;

use support::TestDirectory;

#[test]
fn sync_all_copies_only_relevant_visible_files_and_removes_stale_state() {
    let source = TestDirectory::new("mirror-source");
    let mirror_root = TestDirectory::new("mirror-root");
    fs::create_dir_all(source.path.join("src")).unwrap();
    fs::create_dir_all(source.path.join("node_modules")).unwrap();
    fs::create_dir_all(source.path.join("ignored")).unwrap();
    fs::write(source.path.join("src").join("main.py"), b"value = 1\n").unwrap();
    fs::write(
        source.path.join("src").join("app.ts"),
        b"export const x = 1;\n",
    )
    .unwrap();
    fs::write(source.path.join("README.md"), b"not semantic source\n").unwrap();
    fs::write(
        source.path.join("node_modules").join("lib.js"),
        b"ignored\n",
    )
    .unwrap();
    fs::write(source.path.join(".lexiconignore"), b"ignored/\n").unwrap();
    fs::write(
        source.path.join("ignored").join("skip.go"),
        b"package ignored\n",
    )
    .unwrap();
    fs::write(mirror_root.path.join("stale.py"), b"stale\n").unwrap();

    let mirror = SourceMirror::new(&mirror_root.path);
    mirror.sync_all(&source.path).unwrap();

    assert_eq!(
        mirrored_files(&mirror_root.path),
        vec!["src/app.ts", "src/main.py"]
    );
    assert_eq!(
        fs::read(mirror_root.path.join("src").join("main.py")).unwrap(),
        b"value = 1\n"
    );

    fs::write(source.path.join("src").join("main.py"), b"value = 2\n").unwrap();
    fs::remove_file(source.path.join("src").join("app.ts")).unwrap();
    mirror.sync_all(&source.path).unwrap();
    assert_eq!(mirrored_files(&mirror_root.path), vec!["src/main.py"]);
    assert_eq!(
        fs::read(mirror_root.path.join("src").join("main.py")).unwrap(),
        b"value = 2\n"
    );
}

#[test]
fn sync_paths_updates_directory_scope_and_removes_missing_or_irrelevant_files() {
    let source = TestDirectory::new("mirror-paths-source");
    let mirror_root = TestDirectory::new("mirror-paths-root");
    fs::create_dir_all(source.path.join("src").join("nested")).unwrap();
    fs::write(source.path.join("src").join("a.py"), b"a = 1\n").unwrap();
    fs::write(
        source.path.join("src").join("nested").join("b.go"),
        b"package b\n",
    )
    .unwrap();

    let mirror = SourceMirror::new(&mirror_root.path);
    mirror.sync_all(&source.path).unwrap();

    fs::write(source.path.join("src").join("a.py"), b"a = 2\n").unwrap();
    fs::remove_file(source.path.join("src").join("nested").join("b.go")).unwrap();
    fs::write(source.path.join("src").join("notes.md"), b"irrelevant\n").unwrap();
    mirror
        .sync_paths(&source.path, &[PathBuf::from("src")])
        .unwrap();

    assert_eq!(mirrored_files(&mirror_root.path), vec!["src/a.py"]);
    assert_eq!(
        fs::read(mirror_root.path.join("src").join("a.py")).unwrap(),
        b"a = 2\n"
    );

    fs::remove_file(source.path.join("src").join("a.py")).unwrap();
    mirror
        .sync_paths(&source.path, &[PathBuf::from("src/a.py")])
        .unwrap();
    assert!(mirrored_files(&mirror_root.path).is_empty());
}

fn mirrored_files(root: &std::path::Path) -> Vec<String> {
    let mut result = Vec::new();
    collect(root, root, &mut result);
    result.sort();
    result
}

fn collect(root: &std::path::Path, current: &std::path::Path, result: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, result);
        } else {
            result.push(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

use super::{parser, resolution::DeclarationIndex};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

#[test]
fn same_signature_definition_replaces_prototype_in_resolution_index() {
    let root = TempDirectory::new("definition-replaces-prototype");
    root.write(
        "main.c",
        "int helper(int value);\nint helper(int value) { return value; }\n",
    );

    let model = parser::parse_repository(&root.path).unwrap();
    let index = DeclarationIndex::new(&model);
    let matches = index.resolve("helper", "", "main.c", |value| value.kind == "function");

    assert_eq!(matches.len(), 1);
    assert!(matches[0].definition);
}

#[test]
fn same_file_candidates_precede_cross_file_definitions() {
    let root = TempDirectory::new("same-file-preference");
    root.write("caller.c", "int helper(int value);\n");
    root.write(
        "implementation.c",
        "int helper(int value) { return value + 1; }\n",
    );

    let model = parser::parse_repository(&root.path).unwrap();
    let index = DeclarationIndex::new(&model);
    let matches = index.resolve("helper", "", "caller.c", |value| value.kind == "function");

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].path, "caller.c");
    assert!(!matches[0].definition);
}

#[test]
fn definitions_are_preferred_when_no_same_file_candidate_exists() {
    let root = TempDirectory::new("definition-preference");
    root.write("api.h", "int helper(int value);\n");
    root.write(
        "implementation.c",
        "int helper(int value) { return value + 1; }\n",
    );
    root.write("caller.c", "int run(void) { return helper(1); }\n");

    let model = parser::parse_repository(&root.path).unwrap();
    let index = DeclarationIndex::new(&model);
    let matches = index.resolve("helper", "", "caller.c", |value| value.kind == "function");

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].path, "implementation.c");
    assert!(matches[0].definition);
}

#[test]
fn file_local_candidates_follow_translation_unit_visibility() {
    let root = TempDirectory::new("file-local-visibility");
    root.write("bundle.c", "#include \"helper.c\"\n#include \"user.c\"\n");
    root.write(
        "helper.c",
        "static int helper(int value) { return value + 1; }\n",
    );
    root.write("user.c", "int run(int value) { return helper(value); }\n");
    root.write("other.c", "int other(void) { return 0; }\n");

    let model = parser::parse_repository(&root.path).unwrap();
    let index = DeclarationIndex::new(&model);

    let visible = index.resolve("helper", "", "user.c", |value| value.kind == "function");
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].path, "helper.c");

    let hidden = index.resolve("helper", "", "other.c", |value| value.kind == "function");
    assert!(hidden.is_empty());
}

struct TempDirectory {
    path: PathBuf,
}

impl TempDirectory {
    fn new(name: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lexicon-c-family-resolution-{name}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self
            .path
            .join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

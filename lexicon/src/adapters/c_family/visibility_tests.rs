use super::{includes::FileIndex, parser, visibility::VisibilityIndex};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

#[test]
fn included_c_files_share_translation_unit_for_file_local_visibility() {
    let root = TempDirectory::new("included-c");
    root.write("bundle.c", "#include \"helper.c\"\n#include \"user.c\"\n");
    root.write(
        "helper.c",
        "static int helper(int value) { return value + 1; }\n",
    );
    root.write("user.c", "int run(int value) { return helper(value); }\n");
    root.write("other.c", "int other(void) { return 0; }\n");

    let model = parser::parse_repository(&root.path).unwrap();
    let files = FileIndex::new(&model.files);
    let visibility = VisibilityIndex::new(&model.files, &files);

    let helper = model
        .files
        .iter()
        .find(|file| file.path == "helper.c")
        .unwrap()
        .declarations
        .iter()
        .find(|declaration| declaration.name == "helper")
        .unwrap();
    let run = model
        .files
        .iter()
        .find(|file| file.path == "user.c")
        .unwrap()
        .declarations
        .iter()
        .find(|declaration| declaration.name == "run")
        .unwrap();

    assert!(visibility.declaration_visible("user.c", helper));
    assert!(!visibility.declaration_visible("other.c", helper));
    assert!(visibility.declaration_visible("other.c", run));
    assert_eq!(
        visibility.translation_roots("helper.c"),
        vec!["bundle.c", "helper.c"]
    );
    assert_eq!(
        visibility.translation_roots("user.c"),
        vec!["bundle.c", "user.c"]
    );
}

#[test]
fn included_header_and_source_share_translation_root() {
    let root = TempDirectory::new("header-source");
    root.write(
        "main.c",
        "static int helper(void) { return 42; }\n#include \"fragment.h\"\n",
    );
    root.write(
        "fragment.h",
        "static inline int from_header(void) { return helper(); }\n",
    );

    let model = parser::parse_repository(&root.path).unwrap();
    let files = FileIndex::new(&model.files);
    let visibility = VisibilityIndex::new(&model.files, &files);

    let helper = model
        .files
        .iter()
        .find(|file| file.path == "main.c")
        .unwrap()
        .declarations
        .iter()
        .find(|declaration| declaration.name == "helper")
        .unwrap();
    let from_header = model
        .files
        .iter()
        .find(|file| file.path == "fragment.h")
        .unwrap()
        .declarations
        .iter()
        .find(|declaration| declaration.name == "from_header")
        .unwrap();

    assert!(visibility.declaration_visible("fragment.h", helper));
    assert!(visibility.declaration_visible("main.c", from_header));
    assert_eq!(visibility.translation_roots("fragment.h"), vec!["main.c"]);
}

#[test]
fn include_distance_is_transitive_and_nearest_ranked() {
    let root = TempDirectory::new("include-distance");
    root.write("main.c", "#include \"a.h\"\n");
    root.write("a.h", "#include \"b.h\"\n");
    root.write("b.h", "int answer(void);\n");
    root.write("orphan.h", "int orphan(void);\n");

    let model = parser::parse_repository(&root.path).unwrap();
    let files = FileIndex::new(&model.files);
    let visibility = VisibilityIndex::new(&model.files, &files);

    assert_eq!(visibility.include_rank("main.c", "main.c"), Some(0));
    assert_eq!(visibility.include_rank("main.c", "a.h"), Some(1));
    assert_eq!(visibility.include_rank("main.c", "b.h"), Some(2));
    assert_eq!(visibility.include_rank("a.h", "b.h"), Some(1));
    assert_eq!(visibility.include_rank("main.c", "orphan.h"), None);
    assert!(visibility.translation_roots("orphan.h").is_empty());
}

struct TempDirectory {
    path: PathBuf,
}

impl TempDirectory {
    fn new(name: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lexicon-c-family-visibility-{name}-{}-{}",
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

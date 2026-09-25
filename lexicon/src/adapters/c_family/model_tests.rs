use super::parser;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

#[test]
fn callable_shapes_preserve_defaults_variadics_void_and_unspecified_c() {
    let root = TempDirectory::new("callable-shapes");
    root.write(
        "main.cpp",
        r#"int choose(int value, int fallback = 0);
int route(int first, ...);
"#,
    );
    root.write(
        "legacy.c",
        r#"int unspecified();
int explicit_void(void);
"#,
    );

    let model = parser::parse_repository(&root.path).unwrap();
    let declaration = |name: &str, path: &str| {
        model
            .files
            .iter()
            .find(|file| file.path == path)
            .unwrap()
            .declarations
            .iter()
            .find(|declaration| declaration.name == name)
            .unwrap()
    };

    let choose = declaration("choose", "main.cpp").callable_shape.unwrap();
    assert_eq!(choose.minimum, 1);
    assert_eq!(choose.maximum, Some(2));
    assert!(!choose.variadic);

    let route = declaration("route", "main.cpp").callable_shape.unwrap();
    assert_eq!(route.minimum, 1);
    assert_eq!(route.maximum, None);
    assert!(route.variadic);

    assert!(
        declaration("unspecified", "legacy.c")
            .callable_shape
            .is_none()
    );
    let explicit = declaration("explicit_void", "legacy.c")
        .callable_shape
        .unwrap();
    assert_eq!(explicit.minimum, 0);
    assert_eq!(explicit.maximum, Some(0));
}

struct TempDirectory {
    path: PathBuf,
}

impl TempDirectory {
    fn new(name: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lexicon-c-family-{name}-{}-{}",
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

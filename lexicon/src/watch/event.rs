use std::path::Path;

use notify::event::ModifyKind;
use notify::{Event, EventKind};

use crate::{IGNORE_FILE_NAME, IgnorePolicy};

pub(super) fn is_ignore_file(repository: &Path, path: &Path) -> bool {
    path.strip_prefix(repository)
        .is_ok_and(|relative| relative == Path::new(IGNORE_FILE_NAME))
}

pub(super) fn ignored(policy: &IgnorePolicy, path: &Path) -> bool {
    let is_dir = std::fs::metadata(path).is_ok_and(|metadata| metadata.is_dir());
    policy.ignored(path, is_dir)
}

pub(super) fn relevant(event: &Event, path: &Path) -> bool {
    if destructive(&event.kind) {
        return true;
    }
    if !crate::languages::for_path(path.to_string_lossy().as_ref()).is_empty() {
        return true;
    }
    std::fs::metadata(path).is_ok_and(|metadata| metadata.is_dir())
}

fn destructive(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use notify::event::{CreateKind, RemoveKind};
    use notify::{Event, EventKind};

    use super::*;

    #[test]
    fn ignore_filter_uses_loaded_policy_until_reload() {
        let root = TestDir::new();
        let source = root.path().join("main.go");
        fs::write(&source, b"").unwrap();
        fs::write(root.path().join(IGNORE_FILE_NAME), b"").unwrap();
        let policy = IgnorePolicy::load(root.path()).unwrap();

        fs::write(root.path().join(IGNORE_FILE_NAME), b"main.go\n").unwrap();
        assert!(!ignored(&policy, &source));

        let reloaded = IgnorePolicy::load(root.path()).unwrap();
        assert!(ignored(&reloaded, &source));
    }

    #[test]
    fn relevance_matches_source_directory_and_destructive_events() {
        let root = TestDir::new();
        let go = root.path().join("main.go");
        let text = root.path().join("notes.txt");
        let directory = root.path().join("pkg");
        fs::write(&go, b"").unwrap();
        fs::write(&text, b"").unwrap();
        fs::create_dir(&directory).unwrap();

        let create = Event::new(EventKind::Create(CreateKind::Any));
        assert!(relevant(&create, &go));
        assert!(!relevant(&create, &text));
        assert!(relevant(&create, &directory));

        let remove = Event::new(EventKind::Remove(RemoveKind::Any));
        assert!(relevant(&remove, &text));
    }

    #[test]
    fn recognizes_only_repository_root_ignore_file() {
        let root = TestDir::new();
        assert!(is_ignore_file(
            root.path(),
            &root.path().join(IGNORE_FILE_NAME)
        ));
        assert!(!is_ignore_file(
            root.path(),
            &root.path().join("nested").join(IGNORE_FILE_NAME)
        ));
    }

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "lexicon-watch-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

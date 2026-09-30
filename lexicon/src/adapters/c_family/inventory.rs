use std::collections::BTreeSet;

use crate::{AdapterMode, AdapterRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScanInventory {
    pub(crate) owned_files: Vec<String>,
    pub(crate) context_files: Vec<String>,
}

impl ScanInventory {
    pub(crate) fn from_discovered(
        request: &AdapterRequest,
        mut discovered_files: Vec<String>,
    ) -> Self {
        discovered_files.sort();
        discovered_files.dedup();

        if request.mode == AdapterMode::Full {
            return Self {
                owned_files: discovered_files,
                context_files: Vec::new(),
            };
        }

        let changed = request
            .changed_files
            .iter()
            .filter_map(|path| normalize_relative_path(path))
            .collect::<BTreeSet<_>>();
        let mut owned_files = Vec::new();
        let mut context_files = Vec::new();

        for path in discovered_files {
            if changed.contains(&path) {
                owned_files.push(path);
            } else {
                context_files.push(path);
            }
        }

        Self {
            owned_files,
            context_files,
        }
    }

    pub(crate) fn analysis_files(&self) -> Vec<String> {
        let mut files = self
            .owned_files
            .iter()
            .chain(&self.context_files)
            .cloned()
            .collect::<Vec<_>>();
        files.sort();
        files.dedup();
        files
    }
}

fn normalize_relative_path(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    if path.is_empty() || path.starts_with('/') || path.as_bytes().get(1) == Some(&b':') {
        return None;
    }

    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            value => parts.push(value),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{AdapterMode, AdapterRequest};

    use super::ScanInventory;

    #[test]
    fn full_scan_owns_every_discovered_c_family_file() {
        let request = AdapterRequest {
            language: "c-family".into(),
            mode: AdapterMode::Full,
            repository: PathBuf::from("."),
            ..Default::default()
        };

        let inventory = ScanInventory::from_discovered(
            &request,
            vec![
                "src/main.c".into(),
                "include/api.h".into(),
                "src/main.c".into(),
            ],
        );

        assert_eq!(
            inventory.owned_files,
            ["include/api.h".to_owned(), "src/main.c".to_owned()]
        );
        assert!(inventory.context_files.is_empty());
        assert_eq!(inventory.analysis_files(), inventory.owned_files);
    }

    #[test]
    fn incremental_scan_owns_only_changed_existing_files() {
        let request = AdapterRequest {
            language: "c-family".into(),
            mode: AdapterMode::Incremental,
            repository: PathBuf::from("."),
            changed_files: vec![
                "src\\changed.c".into(),
                "./include/changed.h".into(),
                "removed.c".into(),
                "README.md".into(),
            ],
            removed_files: vec!["removed.c".into()],
            ..Default::default()
        };

        let inventory = ScanInventory::from_discovered(
            &request,
            vec![
                "src/context.c".into(),
                "include/changed.h".into(),
                "src/changed.c".into(),
            ],
        );

        assert_eq!(
            inventory.owned_files,
            ["include/changed.h".to_owned(), "src/changed.c".to_owned()]
        );
        assert_eq!(inventory.context_files, ["src/context.c".to_owned()]);
        assert_eq!(
            inventory.analysis_files(),
            [
                "include/changed.h".to_owned(),
                "src/changed.c".to_owned(),
                "src/context.c".to_owned(),
            ]
        );
    }
}

use std::collections::HashMap;

use super::super::clang_protocol::{FileObservation, SymbolReferenceObservation};

pub(super) type IdentityMaps = HashMap<String, HashMap<String, String>>;

#[derive(Debug)]
pub(super) struct ReferenceIndex {
    by_compiler: HashMap<String, Vec<ReferenceCandidate>>,
}

#[derive(Debug, Clone)]
struct ReferenceCandidate {
    path: String,
    id: String,
    definition: bool,
}

impl ReferenceIndex {
    #[cfg(test)]
    pub(super) fn new(files: &[&FileObservation], ids: &IdentityMaps) -> Self {
        let mut index = Self::empty();
        for file in files {
            if let Some(file_ids) = ids.get(&file.path) {
                index.add_file(file, file_ids);
            }
        }
        index.finalize();
        index
    }

    pub(super) fn empty() -> Self {
        Self {
            by_compiler: HashMap::new(),
        }
    }

    pub(super) fn add_file(&mut self, file: &FileObservation, file_ids: &HashMap<String, String>) {
        for declaration in &file.declarations {
            let Some(id) = file_ids.get(&declaration.compiler_id) else {
                continue;
            };
            self.by_compiler
                .entry(declaration.compiler_id.clone())
                .or_default()
                .push(ReferenceCandidate {
                    path: file.path.clone(),
                    id: id.clone(),
                    definition: declaration.definition,
                });
        }
    }

    pub(super) fn finalize(&mut self) {
        for values in self.by_compiler.values_mut() {
            values.sort_by(|left, right| {
                (!left.definition, left.path.as_str(), left.id.as_str()).cmp(&(
                    !right.definition,
                    right.path.as_str(),
                    right.id.as_str(),
                ))
            });
            values.dedup_by(|left, right| left.id == right.id);
        }
    }

    pub(super) fn resolve(
        &self,
        reference: &SymbolReferenceObservation,
        source_path: &str,
    ) -> String {
        if reference.external {
            return String::new();
        }
        self.resolve_with_path(
            &reference.compiler_id,
            source_path,
            (!reference.path.is_empty()).then_some(reference.path.as_str()),
        )
    }

    pub(super) fn resolve_compiler_id(&self, compiler_id: &str, source_path: &str) -> String {
        self.resolve_with_path(compiler_id, source_path, None)
    }

    fn resolve_with_path(
        &self,
        compiler_id: &str,
        source_path: &str,
        reference_path: Option<&str>,
    ) -> String {
        let Some(candidates) = self.by_compiler.get(compiler_id) else {
            return String::new();
        };
        if let Some(value) = candidates.iter().find(|value| value.path == source_path) {
            return value.id.clone();
        }
        if let Some(value) = candidates.iter().find(|value| value.definition) {
            return value.id.clone();
        }
        if let Some(reference_path) = reference_path
            && let Some(value) = candidates.iter().find(|value| value.path == reference_path)
        {
            return value.id.clone();
        }
        candidates
            .first()
            .map(|value| value.id.clone())
            .unwrap_or_default()
    }
}

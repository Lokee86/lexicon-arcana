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
    pub(super) fn new(files: &[&FileObservation], ids: &IdentityMaps) -> Self {
        let mut by_compiler = HashMap::<String, Vec<ReferenceCandidate>>::new();
        for file in files {
            let Some(file_ids) = ids.get(&file.path) else {
                continue;
            };
            for declaration in &file.declarations {
                let Some(id) = file_ids.get(&declaration.compiler_id) else {
                    continue;
                };
                by_compiler
                    .entry(declaration.compiler_id.clone())
                    .or_default()
                    .push(ReferenceCandidate {
                        path: file.path.clone(),
                        id: id.clone(),
                        definition: declaration.definition,
                    });
            }
        }
        for values in by_compiler.values_mut() {
            values.sort_by(|left, right| {
                (!left.definition, left.path.as_str(), left.id.as_str()).cmp(&(
                    !right.definition,
                    right.path.as_str(),
                    right.id.as_str(),
                ))
            });
            values.dedup_by(|left, right| left.id == right.id);
        }
        Self { by_compiler }
    }

    pub(super) fn resolve(
        &self,
        reference: &SymbolReferenceObservation,
        source_path: &str,
    ) -> String {
        if reference.external {
            return String::new();
        }
        let Some(candidates) = self.by_compiler.get(&reference.compiler_id) else {
            return String::new();
        };
        if let Some(value) = candidates.iter().find(|value| value.path == source_path) {
            return value.id.clone();
        }
        if let Some(value) = candidates.iter().find(|value| value.definition) {
            return value.id.clone();
        }
        if !reference.path.is_empty()
            && let Some(value) = candidates.iter().find(|value| value.path == reference.path)
        {
            return value.id.clone();
        }
        candidates
            .first()
            .map(|value| value.id.clone())
            .unwrap_or_default()
    }
}

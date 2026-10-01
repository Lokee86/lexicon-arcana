use std::collections::HashMap;

use super::{
    super::clang_protocol::{
        ContextIdentityObservation, FileObservation, SymbolReferenceObservation,
    },
    declarations,
};

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
    materialized: bool,
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
                    materialized: true,
                });
        }
    }

    pub(super) fn add_context_identities(&mut self, values: &[ContextIdentityObservation]) {
        for value in values {
            if value.compiler_id.is_empty()
                || value.path.is_empty()
                || value.kind.is_empty()
                || value.qualified_name.is_empty()
            {
                continue;
            }
            self.by_compiler
                .entry(value.compiler_id.clone())
                .or_default()
                .push(ReferenceCandidate {
                    path: value.path.clone(),
                    id: declarations::context_identity_id(value),
                    definition: value.definition,
                    materialized: false,
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

    pub(super) fn resolve_materialized(
        &self,
        reference: &SymbolReferenceObservation,
        source_path: &str,
    ) -> String {
        if reference.external {
            return String::new();
        }
        self.resolve_materialized_with_path(
            &reference.compiler_id,
            source_path,
            (!reference.path.is_empty()).then_some(reference.path.as_str()),
        )
    }

    pub(super) fn resolve_materialized_compiler_id(
        &self,
        compiler_id: &str,
        source_path: &str,
    ) -> String {
        self.resolve_materialized_with_path(compiler_id, source_path, None)
    }

    fn resolve_materialized_with_path(
        &self,
        compiler_id: &str,
        source_path: &str,
        reference_path: Option<&str>,
    ) -> String {
        let Some(candidates) = self.by_compiler.get(compiler_id) else {
            return String::new();
        };
        if let Some(value) = candidates
            .iter()
            .find(|value| value.materialized && value.path == source_path)
        {
            return value.id.clone();
        }
        if let Some(value) = candidates
            .iter()
            .find(|value| value.materialized && value.definition)
        {
            return value.id.clone();
        }
        if let Some(reference_path) = reference_path
            && let Some(value) = candidates
                .iter()
                .find(|value| value.materialized && value.path == reference_path)
        {
            return value.id.clone();
        }
        candidates
            .iter()
            .find(|value| value.materialized)
            .map(|value| value.id.clone())
            .unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::c_family::clang_protocol::{
        ContextIdentityObservation, SymbolReferenceObservation,
    };

    #[test]
    fn context_identity_resolves_to_canonical_declaration_id() {
        let identity = ContextIdentityObservation {
            compiler_id: "c:@F@api".into(),
            path: "api.h".into(),
            kind: "function".into(),
            qualified_name: "api".into(),
            signature: "api()".into(),
            definition: true,
        };
        let expected = declarations::context_identity_id(&identity);

        let mut index = ReferenceIndex::empty();
        index.add_context_identities(std::slice::from_ref(&identity));
        index.finalize();

        let reference = SymbolReferenceObservation {
            compiler_id: identity.compiler_id.clone(),
            path: identity.path.clone(),
            qualified_name: identity.qualified_name.clone(),
            kind: "Function".into(),
            external: false,
        };
        assert_eq!(index.resolve(&reference, "owner.c"), expected);
    }

    #[test]
    fn context_identity_does_not_resolve_as_materialized_source() {
        let identity = ContextIdentityObservation {
            compiler_id: "c:@F@api".into(),
            path: "api.h".into(),
            kind: "function".into(),
            qualified_name: "api".into(),
            signature: "api()".into(),
            definition: true,
        };

        let mut index = ReferenceIndex::empty();
        index.add_context_identities(std::slice::from_ref(&identity));
        index.finalize();

        assert_eq!(
            index.resolve_materialized_compiler_id(&identity.compiler_id, "owner.c"),
            ""
        );
    }
}

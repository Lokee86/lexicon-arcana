use std::collections::BTreeMap;

use serde_json::json;

use super::facts::Facts;
use super::relationships::{PendingRelationship, RelationshipTarget, emit_relationships};
use super::repository::{base_name, path_directory};
use super::runtime::RuntimeIndex;

pub struct AnalysisState {
    pub facts: Facts,
    pub module_by_path: BTreeMap<String, String>,
    pub namespace_by_qn: BTreeMap<String, String>,
    pub pending_relations: Vec<PendingRelationship>,
    pub relationship_by_qn: BTreeMap<String, Vec<RelationshipTarget>>,
    pub repository_id: String,
    pub repository_name: String,
    pub runtime: RuntimeIndex,
}

impl AnalysisState {
    pub fn new(repository_name: String) -> Self {
        let mut facts = Facts::default();
        let repository_id = facts.add_node(
            "repository",
            &repository_name,
            &repository_name,
            ".",
            &repository_name,
            None,
            None,
            None,
        );
        Self {
            facts,
            module_by_path: BTreeMap::new(),
            namespace_by_qn: BTreeMap::new(),
            pending_relations: Vec::new(),
            relationship_by_qn: BTreeMap::new(),
            repository_id,
            repository_name,
            runtime: RuntimeIndex::default(),
        }
    }

    pub fn set_repository_counts(&mut self, source_count: usize, manifest_count: usize) {
        if let Some(attributes) = self.facts.node_attributes_mut(&self.repository_id) {
            *attributes = Some(json!({
                "analysis_mode": "structural",
                "dependency_manifest_count": manifest_count,
                "source_file_count": source_count
            }));
        }
    }

    pub fn emit_directories(&mut self, paths: &[String]) {
        let mut directories = std::collections::BTreeSet::new();
        for path in paths {
            let mut directory = path_directory(path);
            while directory != "." && !directory.is_empty() {
                directories.insert(directory.to_owned());
                directory = path_directory(directory);
            }
        }
        let mut ordered = directories.into_iter().collect::<Vec<_>>();
        ordered.sort_by(|left, right| {
            left.matches('/')
                .count()
                .cmp(&right.matches('/').count())
                .then_with(|| left.cmp(right))
        });
        for directory in ordered {
            let id = self.facts.add_node(
                "directory",
                &directory,
                base_name(&directory),
                &directory,
                &directory,
                None,
                None,
                None,
            );
            let parent = path_directory(&directory);
            let parent_id = if parent == "." || parent.is_empty() {
                self.repository_id.clone()
            } else {
                crate::node_id("kotlin", "directory", parent)
            };
            self.facts
                .add_edge(&parent_id, &id, "contains", None, None, None);
        }
    }

    pub fn parent_directory_id(&self, path: &str) -> String {
        let directory = path_directory(path);
        if directory == "." || directory.is_empty() {
            self.repository_id.clone()
        } else {
            crate::node_id("kotlin", "directory", directory)
        }
    }

    pub fn ensure_namespace(&mut self, qualified: &str) -> String {
        if let Some(existing) = self.namespace_by_qn.get(qualified) {
            return existing.clone();
        }
        let name = qualified.rsplit('.').next().unwrap_or(qualified);
        let id = self.facts.add_node(
            "namespace",
            &format!("package:{qualified}"),
            name,
            ".",
            qualified,
            None,
            None,
            Some(json!({"language_construct": "package"})),
        );
        self.namespace_by_qn.insert(qualified.into(), id.clone());
        self.facts
            .add_edge(&self.repository_id, &id, "contains", None, None, None);
        id
    }

    pub fn emit_relationships(&mut self) {
        emit_relationships(
            &mut self.facts,
            &self.relationship_by_qn,
            &self.pending_relations,
        );
    }
}

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::model::{DependencyEvidence, ManifestFile};
use super::repository::{base_name, path_directory, whole_file_span};
use super::state::AnalysisState;

impl AnalysisState {
    pub fn emit_manifest_facts(&mut self, manifests: &[ManifestFile]) {
        let mut build_modules = BTreeMap::<String, String>::new();
        for manifest in manifests {
            let span = whole_file_span(&manifest.path, &manifest.content);
            let file_id = self
                .facts
                .add_file(&manifest.path, &manifest.content, span.clone());
            let parent = self.parent_directory_id(&manifest.path);
            self.facts.add_edge(
                &parent,
                &file_id,
                "contains",
                Some(&manifest.path),
                None,
                None,
            );
            let source = self.manifest_owner(manifest, &file_id, &mut build_modules);

            let (evidence, parse_error) = match manifest.format.as_str() {
                "gradle-kotlin" | "gradle-groovy" => (
                    super::gradle::parse(&manifest.path, &manifest.content),
                    None,
                ),
                "maven" => super::maven::parse(&manifest.path, &manifest.content),
                _ => (Vec::new(), None),
            };
            if let Some(expression) = parse_error {
                self.facts.add_unresolved(
                    &source,
                    "depends-on",
                    &expression,
                    "unsupported-form",
                    Some(&manifest.path),
                    Some(span.clone()),
                    Some(manifest_attributes(manifest, "")),
                );
            }

            for dependency in evidence {
                let mut attributes =
                    manifest_attributes_object(manifest, &dependency.configuration);
                attributes.insert("expression".into(), json!(dependency.expression));
                if manifest.format == "maven" {
                    attributes.insert("optional".into(), json!(dependency.optional));
                    attributes.insert("scope".into(), json!(dependency.scope));
                }
                if !dependency.resolved {
                    self.facts.add_unresolved(
                        &source,
                        "depends-on",
                        &dependency.expression,
                        "unsupported-form",
                        Some(&manifest.path),
                        Some(dependency.span),
                        Some(Value::Object(attributes)),
                    );
                    continue;
                }
                attributes.insert("coordinate".into(), json!(dependency.coordinate));
                let target = self.external_module(&dependency);
                self.facts.add_edge(
                    &source,
                    &target,
                    "depends-on",
                    Some(&manifest.path),
                    Some(dependency.span),
                    Some(Value::Object(attributes)),
                );
            }
        }
    }

    fn manifest_owner(
        &mut self,
        manifest: &ManifestFile,
        file_id: &str,
        modules: &mut BTreeMap<String, String>,
    ) -> String {
        let directory = path_directory(&manifest.path);
        if directory == "." || directory.is_empty() {
            return self.repository_id.clone();
        }
        if let Some(id) = modules.get(directory) {
            self.facts.add_edge(
                file_id,
                id,
                "defines",
                Some(&manifest.path),
                None,
                Some(json!({"evidence": "dependency-manifest"})),
            );
            return id.clone();
        }
        let canonical = format!("build-module:{directory}");
        let id = self.facts.add_node(
            "module",
            &canonical,
            base_name(directory),
            directory,
            &format!("{}::{canonical}", self.repository_name),
            Some(&manifest.path),
            None,
            Some(json!({"build_module": true})),
        );
        modules.insert(directory.into(), id.clone());
        self.facts.add_edge(
            file_id,
            &id,
            "defines",
            Some(&manifest.path),
            None,
            Some(json!({"evidence": "dependency-manifest"})),
        );
        id
    }

    fn external_module(&mut self, dependency: &DependencyEvidence) -> String {
        let mut attributes = Map::from_iter([
            ("artifact".into(), json!(dependency.artifact)),
            ("dependency".into(), json!(true)),
            ("ecosystem".into(), json!("maven")),
            ("external".into(), json!(true)),
            ("group".into(), json!(dependency.group)),
        ]);
        if !dependency.version.is_empty() {
            attributes.insert("version".into(), json!(dependency.version));
        }
        self.facts.add_node(
            "module",
            &format!("dependency:maven:{}", dependency.coordinate),
            &dependency.artifact,
            &format!(
                "@dependencies/maven/{}",
                dependency.coordinate.replace(':', "/")
            ),
            &dependency.coordinate,
            None,
            None,
            Some(Value::Object(attributes)),
        )
    }
}

fn manifest_attributes(manifest: &ManifestFile, configuration: &str) -> Value {
    Value::Object(manifest_attributes_object(manifest, configuration))
}

fn manifest_attributes_object(manifest: &ManifestFile, configuration: &str) -> Map<String, Value> {
    Map::from_iter([
        ("configuration".into(), json!(configuration)),
        ("evidence".into(), json!("dependency-manifest")),
        ("manifest".into(), json!(manifest.path)),
        ("manifest_type".into(), json!(manifest.format)),
    ])
}

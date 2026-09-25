mod dependencies;
mod diagnostics;
mod emit;
mod facts;
mod gradle;
mod lexer;
mod maven;
mod model;
mod parser;
mod parser_declarations;
mod parser_navigation;
mod parser_parameters;
mod parser_relationships;
mod relationships;
mod repository;
mod runtime;
mod runtime_calls;
mod runtime_dataflow;
mod runtime_extensions;
mod runtime_locals;
mod runtime_overrides;
mod runtime_receiver;
mod runtime_resolution;
mod runtime_tokens;
mod runtime_values;
mod state;
mod tokens;

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::json;

use crate::{
    AdapterError, AdapterRequest, Analysis, FACT_SCHEMA_VERSION, FactHeader, LanguageAdapter,
};

use parser::parse_file;
use repository::{base_name, whole_file_span};
use state::AnalysisState;

pub const ADAPTER_VERSION: &str = "0.4.0";

#[derive(Debug, Default)]
pub struct KotlinAdapter;

impl LanguageAdapter for KotlinAdapter {
    fn implementation_version(&self) -> &'static str {
        ADAPTER_VERSION
    }

    fn implementation_fingerprint(&self) -> String {
        super::fingerprint::source_fingerprint(
            ADAPTER_VERSION,
            &[
                ("mod.rs", include_bytes!("mod.rs")),
                ("dependencies.rs", include_bytes!("dependencies.rs")),
                ("diagnostics.rs", include_bytes!("diagnostics.rs")),
                ("emit.rs", include_bytes!("emit.rs")),
                ("facts.rs", include_bytes!("facts.rs")),
                ("gradle.rs", include_bytes!("gradle.rs")),
                ("lexer.rs", include_bytes!("lexer.rs")),
                ("maven.rs", include_bytes!("maven.rs")),
                ("model.rs", include_bytes!("model.rs")),
                ("parser.rs", include_bytes!("parser.rs")),
                (
                    "parser_declarations.rs",
                    include_bytes!("parser_declarations.rs"),
                ),
                (
                    "parser_navigation.rs",
                    include_bytes!("parser_navigation.rs"),
                ),
                (
                    "parser_parameters.rs",
                    include_bytes!("parser_parameters.rs"),
                ),
                (
                    "parser_relationships.rs",
                    include_bytes!("parser_relationships.rs"),
                ),
                ("relationships.rs", include_bytes!("relationships.rs")),
                ("repository.rs", include_bytes!("repository.rs")),
                ("runtime.rs", include_bytes!("runtime.rs")),
                ("runtime_calls.rs", include_bytes!("runtime_calls.rs")),
                ("runtime_dataflow.rs", include_bytes!("runtime_dataflow.rs")),
                (
                    "runtime_extensions.rs",
                    include_bytes!("runtime_extensions.rs"),
                ),
                ("runtime_locals.rs", include_bytes!("runtime_locals.rs")),
                (
                    "runtime_overrides.rs",
                    include_bytes!("runtime_overrides.rs"),
                ),
                ("runtime_receiver.rs", include_bytes!("runtime_receiver.rs")),
                (
                    "runtime_resolution.rs",
                    include_bytes!("runtime_resolution.rs"),
                ),
                ("runtime_tokens.rs", include_bytes!("runtime_tokens.rs")),
                ("runtime_values.rs", include_bytes!("runtime_values.rs")),
                ("state.rs", include_bytes!("state.rs")),
                ("tokens.rs", include_bytes!("tokens.rs")),
            ],
        )
    }

    fn analyze(&self, request: &AdapterRequest) -> Result<Analysis, AdapterError> {
        if request.language != "kotlin" {
            return Err(AdapterError::new(format!(
                "Kotlin adapter received language {:?}",
                request.language
            )));
        }
        let repository = repository::discover(&request.repository)?;
        let mut state = AnalysisState::new(repository.name.clone());
        state.set_repository_counts(repository.sources.len(), repository.manifests.len());

        let mut paths = repository
            .sources
            .iter()
            .map(|source| source.path.clone())
            .chain(
                repository
                    .manifests
                    .iter()
                    .map(|manifest| manifest.path.clone()),
            )
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        state.emit_directories(&paths);
        state.emit_manifest_facts(&repository.manifests);

        let mut parsed = Vec::new();
        for source in &repository.sources {
            let file = parse_file(&source.path, &source.content);
            let span = whole_file_span(&source.path, &source.content);
            let file_id = state
                .facts
                .add_file(&source.path, &source.content, span.clone());
            let parent = state.parent_directory_id(&source.path);
            state.facts.add_edge(
                &parent,
                &file_id,
                "contains",
                Some(&source.path),
                None,
                None,
            );

            let package = if file.package_name.is_empty() {
                "<default>"
            } else {
                &file.package_name
            };
            let stem = Path::new(&source.path)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or_else(|| base_name(&source.path));
            let module_qn = format!("{package}::source:{}", source.path);
            let module_id = state.facts.add_node(
                "module",
                &format!("source:{}", source.path),
                stem,
                &source.path,
                &module_qn,
                Some(&source.path),
                Some(span.clone()),
                Some(json!({
                    "package": package,
                    "script": source.path.to_ascii_lowercase().ends_with(".kts")
                })),
            );
            state
                .module_by_path
                .insert(source.path.clone(), module_id.clone());
            state.facts.add_edge(
                &file_id,
                &module_id,
                "contains",
                Some(&source.path),
                None,
                None,
            );
            let namespace = state.ensure_namespace(package);
            state.facts.add_edge(
                &module_id,
                &namespace,
                "defines",
                Some(&source.path),
                file.package_span.clone(),
                Some(json!({"evidence": "package-directive"})),
            );
            parsed.push(file);
        }

        for file in &parsed {
            state.emit_imports(file);
            let owner_id = state.module_by_path[&file.path].clone();
            let package = if file.package_name.is_empty() {
                "<default>"
            } else {
                &file.package_name
            };
            let mut occurrences = BTreeMap::new();
            for declaration in &file.declarations {
                state.emit_declaration(
                    file,
                    declaration,
                    &owner_id,
                    package,
                    &format!("source:{}", file.path),
                    "module",
                    &mut occurrences,
                );
            }
            for diagnostic in &file.diagnostics {
                state.facts.add_unresolved(
                    &owner_id,
                    "defines",
                    &diagnostics::expression(&file.content, diagnostic),
                    "unsupported-form",
                    Some(&file.path),
                    Some(crate::SourceSpan {
                        end_column: diagnostic.token.end_column,
                        end_line: diagnostic.token.end_line,
                        path: file.path.clone(),
                        start_column: diagnostic.token.start_column,
                        start_line: diagnostic.token.start_line,
                    }),
                    Some(diagnostics::attributes(diagnostic)),
                );
            }
        }

        state.emit_relationships();
        state.emit_runtime_semantics();

        let incremental = request.mode == crate::AdapterMode::Incremental;
        Ok(Analysis::new(
            FactHeader {
                adapter_version: ADAPTER_VERSION.into(),
                changed_files: incremental.then(|| normalized(&request.changed_files)),
                language: "kotlin".into(),
                mode: incremental.then(|| "incremental".into()),
                record: "lexicon".into(),
                removed_files: incremental.then(|| normalized(&request.removed_files)),
                repository: repository.name,
                schema_version: FACT_SCHEMA_VERSION,
                shared_complete: incremental.then_some(true),
            },
            state.facts.into_records(),
        ))
    }
}

fn normalized(paths: &[String]) -> Vec<String> {
    let mut values = paths
        .iter()
        .map(|path| path.replace('\\', "/"))
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}
